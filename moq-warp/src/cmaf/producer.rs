use bytes::{Buf, Bytes};
use moq_transfork::prelude::*;
use mp4::{self, ReadBox, TrackType};
use std::cmp::max;
use std::collections::HashMap;
use std::io::Cursor;

use super::Error;
use crate::catalog;

pub struct Producer {
	// Broadcast based on their track ID.
	tracks: HashMap<u32, Track>,

	// The full broadcast of tracks
	broadcast: BroadcastWriter,

	// The init and catalog tracks
	init: TrackWriter,
	catalog: catalog::Writer,

	// The ftyp and moov atoms at the start of the file.
	ftyp: Option<Bytes>,
	moov: Option<mp4::MoovBox>,

	// The current track name
	current: Option<u32>,
}

impl Producer {
	pub fn new(mut broadcast: BroadcastWriter) -> Result<Self, Error> {
		let catalog = catalog::Writer::publish(&mut broadcast)?;
		let init = broadcast.build_track("0.mp4", 1).insert()?;

		Ok(Producer {
			tracks: Default::default(),
			broadcast,
			catalog,
			init,
			ftyp: None,
			moov: None,
			current: None,
		})
	}

	// Parse the input buffer, reading any full atoms we can find.
	// Keep appending more data and calling parse.
	pub fn parse<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		while self.parse_atom(buf)? {}
		Ok(())
	}

	fn parse_atom<B: Buf>(&mut self, buf: &mut B) -> Result<bool, Error> {
		let atom = match next_atom(buf)? {
			Some(atom) => atom,
			None => return Ok(false),
		};

		let mut reader = Cursor::new(&atom);
		let header = mp4::BoxHeader::read(&mut reader)?;

		match header.name {
			mp4::BoxType::FtypBox => {
				if self.ftyp.is_some() {
					return Err(Error::DuplicateBox("ftyp"));
				}

				// Save the ftyp atom for later.
				self.ftyp = Some(atom)
			}
			mp4::BoxType::MoovBox => {
				if self.moov.is_some() {
					return Err(Error::DuplicateBox("moov"));
				}

				// Parse the moov box so we can detect the timescales for each track.
				let moov = mp4::MoovBox::read_box(&mut reader, header.size)?;

				self.setup(&moov, atom)?;
				self.moov = Some(moov);
			}
			mp4::BoxType::MoofBox => {
				let moof = mp4::MoofBox::read_box(&mut reader, header.size)?;

				// Process the moof.
				let fragment = Fragment::new(moof)?;

				if fragment.keyframe {
					// Gross but thanks to rust we have to do a separate hashmap lookup
					if self.tracks.get(&fragment.track).ok_or(Error::UnknownTrack)?.handler == TrackType::Video {
						// Start a new group for the keyframe.
						for track in self.tracks.values_mut() {
							track.keyframe();
						}
					}
				}

				// Get the track for this moof.
				let track = self.tracks.get_mut(&fragment.track).ok_or(Error::UnknownTrack)?;

				// Save the track ID for the next iteration, which must be a mdat.
				if self.current.replace(fragment.track).is_some() {
					return Err(Error::DuplicateBox("moof"));
				}

				// Publish the moof header, creating a new segment if it's a keyframe.
				track.header(atom)?;
			}
			mp4::BoxType::MdatBox => {
				// Get the track ID from the previous moof.
				let track = self.current.take().ok_or(Error::MissingBox("moof"))?;
				let track = self.tracks.get_mut(&track).ok_or(Error::UnknownTrack)?;

				// Publish the mdat atom.
				track.data(atom)?;
			}

			_ => {
				// Skip unknown atoms
			}
		}

		Ok(true)
	}

	fn setup(&mut self, moov: &mp4::MoovBox, raw: Bytes) -> Result<(), Error> {
		// Combine the ftyp+moov atoms into a single object.
		let mut init = self.ftyp.clone().ok_or(Error::MissingBox("ftyp"))?.to_vec();
		init.extend_from_slice(&raw);

		// Create the catalog track with a single segment.
		self.init.append_group()?.write_frame(init.into())?;

		let mut tracks = Vec::new();

		// Produce the catalog
		for trak in &moov.traks {
			let id = trak.tkhd.track_id;
			let name = format!("{}.m4s", id);

			let handler = (&trak.mdia.hdlr.handler_type).try_into()?;

			let mut selection_params = catalog::SelectionParam::default();

			let mut track = catalog::Track {
				init_track: Some(self.init.name.clone()),
				name: name.clone(),
				namespace: self.broadcast.name.clone(),
				packaging: Some(catalog::TrackPackaging::Cmaf),
				render_group: Some(1),
				..Default::default()
			};

			let stsd = &trak.mdia.minf.stbl.stsd;

			if let Some(avc1) = &stsd.avc1 {
				// avc1[.PPCCLL]
				//
				// let profile = 0x64;
				// let constraints = 0x00;
				// let level = 0x1f;
				let profile = avc1.avcc.avc_profile_indication;
				let constraints = avc1.avcc.profile_compatibility; // Not 100% certain here, but it's 0x00 on my current test video
				let level = avc1.avcc.avc_level_indication;

				let width = avc1.width;
				let height = avc1.height;

				let codec = rfc6381_codec::Codec::avc1(profile, constraints, level);
				let codec_str = codec.to_string();

				selection_params.codec = Some(codec_str);
				selection_params.width = Some(width.into());
				selection_params.height = Some(height.into());
			} else if let Some(_hev1) = &stsd.hev1 {
				// TODO https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L106
				return Err(Error::UnsupportedCodec("HEVC"));
			} else if let Some(mp4a) = &stsd.mp4a {
				let desc = &mp4a.esds.as_ref().ok_or(Error::MissingBox("esds"))?.es_desc.dec_config;
				let codec_str = format!("mp4a.{:02x}.{}", desc.object_type_indication, desc.dec_specific.profile);

				selection_params.codec = Some(codec_str);
				selection_params.channel_config = Some(mp4a.channelcount.to_string());
				selection_params.samplerate = Some(mp4a.samplerate.value().into());

				let bitrate = max(desc.max_bitrate, desc.avg_bitrate);
				if bitrate > 0 {
					selection_params.bitrate = Some(bitrate);
				}
			} else if let Some(vp09) = &stsd.vp09 {
				// https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L238
				let vpcc = &vp09.vpcc;
				let codec_str = format!("vp09.0.{:02x}.{:02x}.{:02x}", vpcc.profile, vpcc.level, vpcc.bit_depth);

				selection_params.codec = Some(codec_str);
				selection_params.width = Some(vp09.width.into());
				selection_params.height = Some(vp09.height.into());

				// TODO Test if this actually works; I'm just guessing based on mp4box.js
				return Err(Error::UnsupportedCodec("VP9"));
			} else {
				// TODO add av01 support: https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L251
				return Err(Error::UnsupportedCodec("unknown"));
			}

			track.selection_params = selection_params;

			tracks.push(track);

			// Change the track priority based on the media type
			let priority = match handler {
				TrackType::Video => 4,
				TrackType::Audio => 3,
				TrackType::Subtitle => 2,
			};

			// Store the track publisher in a map so we can update it later.
			let track = self.broadcast.build_track(&name, priority).insert()?;

			let track = Track::new(track, handler);
			self.tracks.insert(id, track);
		}

		let catalog = catalog::Root {
			version: 1,
			streaming_format: 1,
			streaming_format_version: "0.2".to_string(),
			streaming_delta_updates: true,
			tracks,
		};

		tracing::info!(?catalog);

		// Create a single fragment for the segment.
		self.catalog.write(catalog)?;

		Ok(())
	}
}

// Find the next full atom in the buffer.
// TODO return the amount of data still needed in Err?
fn next_atom<B: Buf>(buf: &mut B) -> Result<Option<Bytes>, Error> {
	let mut peek = Cursor::new(buf.chunk());

	if peek.remaining() < 8 {
		if buf.remaining() != buf.chunk().len() {
			// TODO figure out a way to peek at the first 8 bytes
			unimplemented!("TODO: vectored Buf not yet supported");
		}

		return Ok(None);
	}

	// Convert the first 4 bytes into the size.
	let size = peek.get_u32();
	let _type = peek.get_u32();

	let size = match size {
		// Runs until the end of the file.
		0 => unimplemented!("TODO: unsupported EOF atom"),

		// The next 8 bytes are the extended size to be used instead.
		1 => {
			let size_ext = peek.get_u64();
			if size_ext < 16 {
				return Err(Error::InvalidSize);
			}
			size_ext as usize
		}

		2..=7 => {
			return Err(Error::InvalidSize);
		}

		size => size as usize,
	};

	if buf.remaining() < size {
		return Ok(None);
	}

	let atom = buf.copy_to_bytes(size);

	Ok(Some(atom))
}

struct Track {
	// The track we're producing
	track: TrackWriter,

	// The current group of pictures
	group: Option<GroupWriter>,

	// The moof header
	header: Option<Bytes>,

	// The type of track, ex. "vide" or "soun"
	handler: TrackType,
}

impl Track {
	fn new(track: TrackWriter, handler: TrackType) -> Self {
		Self {
			track,
			group: None,
			header: None,
			handler,
		}
	}

	pub fn header(&mut self, moof: Bytes) -> Result<(), Error> {
		if self.header.is_some() {
			return Err(Error::DuplicateBox("moof"));
		}

		self.header = Some(moof);
		Ok(())
	}

	pub fn data(&mut self, mdat: Bytes) -> Result<(), Error> {
		let moof = self.header.take().ok_or(Error::MissingBox("moof"))?;

		let mut group = match self.group.take() {
			Some(group) => group,
			None => self.track.append_group()?,
		};

		let mut frame = group.create_frame(mdat.len() + moof.len())?;
		frame.write_chunk(moof)?;
		frame.write_chunk(mdat)?;

		self.group.replace(group);

		Ok(())
	}

	pub fn keyframe(&mut self) {
		self.group = None;
	}
}

struct Fragment {
	// The track for this fragment.
	track: u32,

	// True if this fragment is a keyframe.
	keyframe: bool,
}

impl Fragment {
	fn new(moof: mp4::MoofBox) -> Result<Self, Error> {
		// We can't split the mdat atom, so this is impossible to support
		if moof.trafs.len() != 1 {
			return Err(Error::DuplicateBox("traf"));
		}

		let track = moof.trafs[0].tfhd.track_id;

		// Detect if we should start a new segment.
		let keyframe = sample_keyframe(&moof);

		Ok(Self { track, keyframe })
	}
}

fn sample_keyframe(moof: &mp4::MoofBox) -> bool {
	for traf in &moof.trafs {
		// TODO trak default flags if this is None
		let default_flags = traf.tfhd.default_sample_flags.unwrap_or_default();
		let trun = match &traf.trun {
			Some(t) => t,
			None => return false,
		};

		for i in 0..trun.sample_count {
			let mut flags = match trun.sample_flags.get(i as usize) {
				Some(f) => *f,
				None => default_flags,
			};

			if i == 0 && trun.first_sample_flags.is_some() {
				flags = trun.first_sample_flags.unwrap();
			}

			// https://chromium.googlesource.com/chromium/src/media/+/master/formats/mp4/track_run_iterator.cc#177
			let keyframe = (flags >> 24) & 0x3 == 0x2; // kSampleDependsOnNoOther
			let non_sync = (flags >> 16) & 0x1 == 0x1; // kSampleIsNonSyncSample

			if keyframe && !non_sync {
				return true;
			}
		}
	}

	false
}
