use anyhow::{self, Context};
use moq_transport::serve::{GroupWriter, GroupsWriter, TrackWriter, TracksWriter};
use mp4::{self, ReadBox, TrackType};
use serde_json::json;
use std::cmp::max;
use std::collections::HashMap;
use std::io::Cursor;
use std::time;
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct Media<I> {
	// Tracks based on their track ID.
	tracks: HashMap<u32, Track>,
	input: I,
}

impl<I: AsyncRead + Send + Unpin> Media<I> {
	pub async fn new(mut input: I, mut broadcast: TracksWriter) -> anyhow::Result<Self> {
		let ftyp = read_atom(&mut input).await?;
		anyhow::ensure!(&ftyp[4..8] == b"ftyp", "expected ftyp atom");

		let moov = read_atom(&mut input).await?;
		anyhow::ensure!(&moov[4..8] == b"moov", "expected moov atom");

		let mut init = ftyp;
		init.extend(&moov);

		// We're going to parse the moov box.
		// We have to read the moov box header to correctly advance the cursor for the mp4 crate.
		let mut moov_reader = Cursor::new(&moov);
		let moov_header = mp4::BoxHeader::read(&mut moov_reader)?;

		// Parse the moov box so we can detect the timescales for each track.
		let moov = mp4::MoovBox::read_box(&mut moov_reader, moov_header.size)?;

		// Create the catalog track with a single segment.
		let mut init_track = broadcast.create("0.mp4").context("broadcast closed")?.groups()?;
		init_track.append(0)?.write(init.into())?;

		let mut tracks = HashMap::new();

		for trak in &moov.traks {
			let id = trak.tkhd.track_id;
			let name = format!("{}.m4s", id);

			let timescale = track_timescale(&moov, id);
			let handler = (&trak.mdia.hdlr.handler_type).try_into()?;

			// Store the track publisher in a map so we can update it later.
			let track = broadcast.create(&name).context("broadcast closed")?;
			let track = Track::new(track, handler, timescale);
			tracks.insert(id, track);
		}

		let catalog = broadcast.create(".catalog").context("broadcast closed")?;

		// Create the catalog track
		Self::serve_catalog(catalog, &init_track.name, &moov)?;

		Ok(Media { tracks, input })
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		// The current track name
		let mut current = None;

		loop {
			let atom = read_atom(&mut self.input).await?;

			let mut reader = Cursor::new(&atom);
			let header = mp4::BoxHeader::read(&mut reader)?;

			match header.name {
				mp4::BoxType::MoofBox => {
					let moof = mp4::MoofBox::read_box(&mut reader, header.size).context("failed to read MP4")?;

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
					anyhow::ensure!(current.is_none(), "multiple moof atoms");
					current.replace(fragment.track);

					// Publish the moof header, creating a new segment if it's a keyframe.
					track.header(atom, fragment).context("failed to publish moof")?;
				}
				mp4::BoxType::MdatBox => {
					// Get the track ID from the previous moof.
					let track = current.take().context("missing moof")?;
					let track = self.tracks.get_mut(&track).context("failed to find track")?;

					// Publish the mdat atom.
					track.data(atom).context("failed to publish mdat")?;
				}

				_ => {
					// Skip unknown atoms
				}
			}
		}
	}

	fn serve_catalog(track: TrackWriter, init_track_name: &str, moov: &mp4::MoovBox) -> Result<(), anyhow::Error> {
		let mut segment = track.groups()?.append(0)?;

		let mut tracks = Vec::new();

		for trak in &moov.traks {
			let mut track = json!({
				"container": "mp4",
				"init_track": init_track_name,
				"data_track": format!("{}.m4s", trak.tkhd.track_id),
			});

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

				track["kind"] = json!("video");
				track["codec"] = json!(codec_str);
				track["width"] = json!(width);
				track["height"] = json!(height);
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

				track["kind"] = json!("audio");
				track["codec"] = json!(codec_str);
				track["channel_count"] = json!(mp4a.channelcount);
				track["sample_rate"] = json!(mp4a.samplerate.value());
				track["sample_size"] = json!(mp4a.samplesize);

				let bitrate = max(desc.max_bitrate, desc.avg_bitrate);
				if bitrate > 0 {
					track["bit_rate"] = json!(bitrate);
				}
			} else if let Some(vp09) = &stsd.vp09 {
				// https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L238
				let vpcc = &vp09.vpcc;
				let codec_str = format!("vp09.0.{:02x}.{:02x}.{:02x}", vpcc.profile, vpcc.level, vpcc.bit_depth);

				track["kind"] = json!("video");
				track["codec"] = json!(codec_str);
				track["width"] = json!(vp09.width); // no idea if this needs to be multiplied
				track["height"] = json!(vp09.height); // no idea if this needs to be multiplied

				// TODO Test if this actually works; I'm just guessing based on mp4box.js
				anyhow::bail!("VP9 not yet supported")
			} else {
				// TODO add av01 support: https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L251
				anyhow::bail!("unknown codec for track: {}", trak.tkhd.track_id);
			}

			tracks.push(track);
		}

		let catalog = json!({
			"tracks": tracks
		});

		let catalog_str = serde_json::to_string_pretty(&catalog)?;
		log::info!("catalog: {}", catalog_str);

		// Create a single fragment for the segment.
		segment.write(catalog_str.into())?;

		Ok(())
	}
}

// Read a full MP4 atom into a vector.
async fn read_atom<R: AsyncReadExt + Unpin>(reader: &mut R) -> anyhow::Result<Vec<u8>> {
	// Read the 8 bytes for the size + type
	let mut buf = [0u8; 8];
	reader.read_exact(&mut buf).await?;

	// Convert the first 4 bytes into the size.
	let size = u32::from_be_bytes(buf[0..4].try_into()?) as u64;

	let mut raw = buf.to_vec();

	let mut limit = match size {
		// Runs until the end of the file.
		0 => reader.take(u64::MAX),

		// The next 8 bytes are the extended size to be used instead.
		1 => {
			reader.read_exact(&mut buf).await?;
			let size_large = u64::from_be_bytes(buf);
			anyhow::ensure!(size_large >= 16, "impossible extended box size: {}", size_large);

			reader.take(size_large - 16)
		}

		2..=7 => {
			anyhow::bail!("impossible box size: {}", size)
		}

		size => reader.take(size - 8),
	};

	// Append to the vector and return it.
	let _read_bytes = limit.read_to_end(&mut raw).await?;

	Ok(raw)
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

	pub fn header(&mut self, raw: Vec<u8>, fragment: Fragment) -> anyhow::Result<()> {
		if let Some(current) = self.current.as_mut() {
			// Use the existing segment
			current.write(raw.into())?;
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
		segment.write(raw.into())?;

		// Save for the next iteration
		self.current = Some(segment);

		Ok(())
	}

	pub fn data(&mut self, raw: Vec<u8>) -> anyhow::Result<()> {
		let segment = self.current.as_mut().context("missing current fragment")?;
		segment.write(raw.into())?;

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
