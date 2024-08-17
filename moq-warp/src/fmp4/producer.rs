use bytes::{Buf, Bytes};
use moq_transfork::prelude::*;
use mp4::{self, ReadBox, TrackType};
use std::collections::HashMap;
use std::io::Cursor;

use super::Error;
use crate::catalog;

pub struct Producer {
	// Broadcast based on their track ID.
	tracks: HashMap<u32, Track>,

	// The full broadcast of tracks
	broadcast: BroadcastProducer,

	// The ftyp and moov atoms at the start of the file.
	ftyp: Option<Bytes>,
	moov: Option<mp4::MoovBox>,

	// The current track name
	current: Option<u32>,
}

impl Producer {
	pub fn new(broadcast: BroadcastProducer) -> Result<Self, Error> {
		Ok(Producer {
			tracks: Default::default(),
			broadcast,
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

		let mut catalog = catalog::Broadcast {
			video: Default::default(),
			audio: Default::default(),
			init: HashMap::from([(catalog::Container::Fmp4, init)]),
		};

		// Produce the catalog
		for trak in &moov.traks {
			let id = trak.tkhd.track_id;
			let name = format!("{}.m4s", id);

			let handler = (&trak.mdia.hdlr.handler_type).try_into()?;

			// I hate this mp4 library.
			// The vast majority of boxes are NOT exported so we need to use workarounds.
			// In this case, we're creating a Mp4Track object just to pass trak to the setup functions.
			let trak = mp4::Mp4Track {
				trak: trak.clone(),
				trafs: vec![],
				default_sample_duration: 0,
			};

			match handler {
				TrackType::Video => {
					let video = Self::setup_video(&name, &trak)?;

					let producer = self.broadcast.insert_track(video.track.clone());
					let track = Track::new(producer, handler);
					self.tracks.insert(id, track);

					catalog.video.push(video);
				}
				TrackType::Audio => {
					let audio = Self::setup_audio(&name, &trak)?;

					let producer = self.broadcast.insert_track(audio.track.clone());
					let track = Track::new(producer, handler);
					self.tracks.insert(id, track);

					catalog.audio.push(audio);
				}
				TrackType::Subtitle => {
					return Err(Error::UnsupportedTrack("subtitle"));
				}
			}
		}

		tracing::info!(?catalog);

		catalog.publish(&mut self.broadcast)?;

		Ok(())
	}

	fn setup_video(name: &str, track: &mp4::Mp4Track) -> Result<catalog::Video, Error> {
		// NOTE: mp4 does not export these box types so it's a pain in the but to refactor.
		let stsd = &track.trak.mdia.minf.stbl.stsd;

		let track = if let Some(avc1) = &stsd.avc1 {
			let avcc = &avc1.avcc;
			catalog::Video {
				track: moq_transfork::Track::build(name).priority(2).into(),
				dimensions: catalog::Dimensions {
					width: avc1.width,
					height: avc1.height,
				},
				display: Some(catalog::Dimensions {
					width: avc1.horizresolution.value(),
					height: avc1.vertresolution.value(),
				}),
				codec: catalog::H264 {
					profile: avcc.avc_profile_indication,
					constraints: avcc.profile_compatibility,
					level: avcc.avc_level_indication,
				}
				.into(),
				container: catalog::Container::Fmp4,
				layers: vec![],
				bitrate: None,
			}
		} else if let Some(hev1) = &stsd.hev1 {
			let _hvcc = &hev1.hvcc;

			/*
			catalog::Video {
				track: moq_transfork::Track::build(name).priority(2).into(),
				width: hev1.width,
				height: hev1.height,
				codec: catalog::H265 {
					profile: hvcc.general_profile_idc,
					level: hvcc.general_level_idc,
					constraints: hvcc.general_constraint_indicator_flag,
				},
				container: catalog::Container::Fmp4,
				layers: vec![],
				bit_rate: None,
			}
			*/

			// Just waiting for a release:
			// https://github.com/alfg/mp4-rust/commit/35560e94f5e871a2b2d88bfe964013b39af131e8
			return Err(Error::UnsupportedCodec("HEVC"));
		} else if let Some(vp09) = &stsd.vp09 {
			// https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L238
			let vpcc = &vp09.vpcc;

			catalog::Video {
				track: moq_transfork::Track::build(name).priority(2).into(),
				codec: catalog::VP9 {
					profile: vpcc.profile,
					level: vpcc.level,
					bit_depth: vpcc.bit_depth,
					chroma_subsampling: vpcc.chroma_subsampling,
					color_primaries: vpcc.color_primaries,
					transfer_characteristics: vpcc.transfer_characteristics,
					matrix_coefficients: vpcc.matrix_coefficients,
					full_range: vpcc.video_full_range_flag,
				}
				.into(),
				container: catalog::Container::Fmp4,
				dimensions: catalog::Dimensions {
					width: vp09.width,
					height: vp09.height,
				},
				display: None,
				layers: vec![],
				bitrate: None,
			}
		} else {
			// TODO add av01 support: https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L251
			return Err(Error::UnsupportedCodec("unknown"));
		};

		Ok(track)
	}

	fn setup_audio(name: &str, track: &mp4::Mp4Track) -> Result<catalog::Audio, Error> {
		// NOTE: mp4 does not export these box types so it's a pain in the but to refactor.
		let stsd = &track.trak.mdia.minf.stbl.stsd;

		let track = if let Some(mp4a) = &stsd.mp4a {
			let desc = &mp4a.esds.as_ref().ok_or(Error::MissingBox("esds"))?.es_desc.dec_config;

			// TODO Also support mp4a.67
			if desc.object_type_indication != 0x40 {
				return Err(Error::UnsupportedCodec("MPEG2"));
			}

			catalog::Audio {
				track: moq_transfork::Track::build(name).priority(1).into(),
				codec: catalog::AAC {
					profile: desc.dec_specific.profile,
				}
				.into(),
				container: catalog::Container::Fmp4,
				sample_rate: mp4a.samplerate.value(),
				channel_count: mp4a.channelcount,
				bitrate: Some(std::cmp::max(desc.avg_bitrate, desc.max_bitrate)),
			}
		} else {
			return Err(Error::UnsupportedCodec("unknown"));
		};

		Ok(track)
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
	track: TrackProducer,

	// The current group of pictures
	group: Option<GroupProducer>,

	// The moof header
	header: Option<Bytes>,

	// The type of track, ex. "vide" or "soun"
	handler: TrackType,
}

impl Track {
	fn new(track: TrackProducer, handler: TrackType) -> Self {
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
			None => self.track.append_group(),
		};

		let mut frame = group.create_frame(mdat.len() + moof.len());
		frame.write(moof);
		frame.write(mdat);

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
