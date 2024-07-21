use anyhow::{self, Context};
use bytes::{Buf, Bytes};
use moq_transport::serve::{GroupWriter, GroupsWriter, TrackWriter, TracksWriter};
use mp4::{self, ReadBox, TrackType};
use std::cmp::max;
use std::collections::HashMap;
use std::io::Cursor;
use std::time;

pub struct Media {
	// Tracks based on their track ID.
	tracks: HashMap<u32, Track>,

	// The full broadcast of tracks
	broadcast: TracksWriter,

	// The init and catalog tracks
	init: GroupsWriter,
	catalog: GroupsWriter,

	// The ftyp and moov atoms at the start of the file.
	ftyp: Option<Bytes>,
	moov: Option<mp4::MoovBox>,

	// The current track name
	current: Option<u32>,
}

impl Media {
	pub fn new(mut broadcast: TracksWriter) -> anyhow::Result<Self> {
		let catalog = broadcast.create(".catalog").context("broadcast closed")?.groups()?;
		let init = broadcast.create("0.mp4").context("broadcast closed")?.groups()?;

		Ok(Media {
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
	pub fn parse<B: Buf>(&mut self, buf: &mut B) -> anyhow::Result<()> {
		while self.parse_atom(buf)? {}
		Ok(())
	}

	fn parse_atom<B: Buf>(&mut self, buf: &mut B) -> anyhow::Result<bool> {
		let atom = match next_atom(buf)? {
			Some(atom) => atom,
			None => return Ok(false),
		};

		let mut reader = Cursor::new(&atom);
		let header = mp4::BoxHeader::read(&mut reader)?;

		match header.name {
			mp4::BoxType::FtypBox => {
				if self.ftyp.is_some() {
					anyhow::bail!("multiple ftyp atoms");
				}

				// Save the ftyp atom for later.
				self.ftyp = Some(atom)
			}
			mp4::BoxType::MoovBox => {
				if self.moov.is_some() {
					anyhow::bail!("multiple moov atoms");
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
					if self
						.tracks
						.get(&fragment.track)
						.context("failed to find track")?
						.handler == TrackType::Video
					{
						// Start a new group for the keyframe.
						for track in self.tracks.values_mut() {
							track.end_group();
						}
					}
				}

				// Get the track for this moof.
				let track = self.tracks.get_mut(&fragment.track).context("failed to find track")?;

				// Save the track ID for the next iteration, which must be a mdat.
				anyhow::ensure!(self.current.is_none(), "multiple moof atoms");
				self.current.replace(fragment.track);

				// Publish the moof header, creating a new segment if it's a keyframe.
				track.header(atom, fragment).context("failed to publish moof")?;
			}
			mp4::BoxType::MdatBox => {
				// Get the track ID from the previous moof.
				let track = self.current.take().context("missing moof")?;
				let track = self.tracks.get_mut(&track).context("failed to find track")?;

				// Publish the mdat atom.
				track.data(atom).context("failed to publish mdat")?;
			}

			_ => {
				// Skip unknown atoms
			}
		}

		Ok(true)
	}

	fn setup(&mut self, moov: &mp4::MoovBox, raw: Bytes) -> anyhow::Result<()> {
		// Create a track for each track in the moov
		for trak in &moov.traks {
			let id = trak.tkhd.track_id;
			let name = format!("{}.m4s", id);

			let timescale = track_timescale(moov, id);
			let handler = (&trak.mdia.hdlr.handler_type).try_into()?;

			// Store the track publisher in a map so we can update it later.
			let track = self.broadcast.create(&name).context("broadcast closed")?;
			let track = Track::new(track, handler, timescale);
			self.tracks.insert(id, track);
		}

		// Combine the ftyp+moov atoms into a single object.
		let mut init = self.ftyp.clone().context("missing ftyp")?.to_vec();
		init.extend_from_slice(&raw);

		// Create the catalog track with a single segment.
		self.init.append(0)?.write(init.into())?;

		let mut tracks = Vec::new();

		// Produce the catalog
		for trak in &moov.traks {
			let mut selection_params = moq_catalog::SelectionParam::default();

			let mut track = moq_catalog::Track {
				init_track: Some("0.mp4".to_string()),
				data_track: Some(format!("{}.m4s", trak.tkhd.track_id)),
				namespace: Some(self.broadcast.namespace.clone()),
				packaging: Some(moq_catalog::TrackPackaging::Cmaf),
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

				track.name = format!("video_{}p", height);
				selection_params.codec = Some(codec_str);
				selection_params.width = Some(width.into());
				selection_params.height = Some(height.into());
			} else if let Some(_hev1) = &stsd.hev1 {
				// TODO https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L106
				anyhow::bail!("HEVC not yet supported")
			} else if let Some(mp4a) = &stsd.mp4a {
				let desc = &mp4a
					.esds
					.as_ref()
					.context("missing esds box for MP4a")?
					.es_desc
					.dec_config;
				let codec_str = format!("mp4a.{:02x}.{}", desc.object_type_indication, desc.dec_specific.profile);

				track.name = "audio".to_string();
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

				track.name = format!("video_{}p", vp09.height);
				selection_params.codec = Some(codec_str);
				selection_params.width = Some(vp09.width.into());
				selection_params.height = Some(vp09.height.into());

				// TODO Test if this actually works; I'm just guessing based on mp4box.js
				anyhow::bail!("VP9 not yet supported")
			} else {
				// TODO add av01 support: https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L251
				anyhow::bail!("unknown codec for track: {}", trak.tkhd.track_id);
			}

			track.selection_params = selection_params;

			tracks.push(track);
		}

		let catalog = moq_catalog::Root {
			version: 1,
			streaming_format: 1,
			streaming_format_version: "0.2".to_string(),
			streaming_delta_updates: true,
			common_track_fields: moq_catalog::CommonTrackFields::from_tracks(&mut tracks),
			tracks,
		};

		let catalog_str = serde_json::to_string_pretty(&catalog)?;

		log::info!("catalog: {}", catalog_str);

		// Create a single fragment for the segment.
		self.catalog.append(0)?.write(catalog_str.into())?;

		Ok(())
	}
}

// Find the next full atom in the buffer.
// TODO return the amount of data still needed in Err?
fn next_atom<B: Buf>(buf: &mut B) -> anyhow::Result<Option<Bytes>> {
	let mut peek = Cursor::new(buf.chunk());

	if peek.remaining() < 8 {
		if buf.remaining() != buf.chunk().len() {
			// TODO figure out a way to peek at the first 8 bytes
			anyhow::bail!("TODO: vectored Buf not yet supported");
		}

		return Ok(None);
	}

	// Convert the first 4 bytes into the size.
	let size = peek.get_u32();
	let _type = peek.get_u32();

	let size = match size {
		// Runs until the end of the file.
		0 => anyhow::bail!("TODO: unsupported EOF atom"),

		// The next 8 bytes are the extended size to be used instead.
		1 => {
			let size_ext = peek.get_u64();
			anyhow::ensure!(size_ext >= 16, "impossible extended box size: {}", size_ext);
			size_ext as usize
		}

		2..=7 => {
			anyhow::bail!("impossible box size: {}", size)
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
	track: GroupsWriter,

	// The current segment
	current: Option<GroupWriter>,

	// The number of units per second.
	timescale: u64,

	// The type of track, ex. "vide" or "soun"
	handler: TrackType,
}

impl Track {
	fn new(track: TrackWriter, handler: TrackType, timescale: u64) -> Self {
		Self {
			track: track.groups().unwrap(),
			current: None,
			timescale,
			handler,
		}
	}

	pub fn header(&mut self, raw: Bytes, fragment: Fragment) -> anyhow::Result<()> {
		if let Some(current) = self.current.as_mut() {
			// Use the existing segment
			current.write(raw)?;
			return Ok(());
		}

		// Otherwise make a new segment

		// Compute the timestamp in milliseconds.
		// Overflows after 583 million years, so we're fine.
		let timestamp: u32 = fragment
			.timestamp(self.timescale)
			.as_millis()
			.try_into()
			.context("timestamp too large")?;

		let priority = u32::MAX.checked_sub(timestamp).context("priority too large")?.into();

		// Create a new segment.
		let mut segment = self.track.append(priority)?;

		// Write the fragment in it's own object.
		segment.write(raw)?;

		// Save for the next iteration
		self.current = Some(segment);

		Ok(())
	}

	pub fn data(&mut self, raw: Bytes) -> anyhow::Result<()> {
		let segment = self.current.as_mut().context("missing current fragment")?;
		segment.write(raw)?;

		Ok(())
	}

	pub fn end_group(&mut self) {
		self.current = None;
	}
}

struct Fragment {
	// The track for this fragment.
	track: u32,

	// The timestamp of the first sample in this fragment, in timescale units.
	timestamp: u64,

	// True if this fragment is a keyframe.
	keyframe: bool,
}

impl Fragment {
	fn new(moof: mp4::MoofBox) -> anyhow::Result<Self> {
		// We can't split the mdat atom, so this is impossible to support
		anyhow::ensure!(moof.trafs.len() == 1, "multiple tracks per moof atom");
		let track = moof.trafs[0].tfhd.track_id;

		// Parse the moof to get some timing information to sleep.
		let timestamp = sample_timestamp(&moof).expect("couldn't find timestamp");

		// Detect if we should start a new segment.
		let keyframe = sample_keyframe(&moof);

		Ok(Self {
			track,
			timestamp,
			keyframe,
		})
	}

	// Convert from timescale units to a duration.
	fn timestamp(&self, timescale: u64) -> time::Duration {
		time::Duration::from_millis(1000 * self.timestamp / timescale)
	}
}

fn sample_timestamp(moof: &mp4::MoofBox) -> Option<u64> {
	Some(moof.trafs.first()?.tfdt.as_ref()?.base_media_decode_time)
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

// Find the timescale for the given track.
fn track_timescale(moov: &mp4::MoovBox, track_id: u32) -> u64 {
	let trak = moov
		.traks
		.iter()
		.find(|trak| trak.tkhd.track_id == track_id)
		.expect("failed to find trak");

	trak.mdia.mdhd.timescale as u64
}
