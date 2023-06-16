use std::io::Read;

use std::{fs, io, path, time};

use anyhow;

use mp4;
use mp4::ReadBox;

use anyhow::Context;
use std::collections::HashMap;
use std::sync::Arc;

use moq_transport::coding::VarInt;

use super::{Broadcast, Fragment, Producer, Segment, Track};

pub struct Source {
	// We read the file once, in order, and don't seek backwards.
	reader: io::BufReader<fs::File>,

	// The subscribable broadcast.
	broadcast: Broadcast,

	// The tracks we're producing.
	tracks: HashMap<u32, SourceTrack>,
}

impl Source {
	pub fn new(path: path::PathBuf) -> anyhow::Result<Self> {
		let f = fs::File::open(path)?;
		let mut reader = io::BufReader::new(f);

		let ftyp = read_atom(&mut reader)?;
		anyhow::ensure!(&ftyp[4..8] == b"ftyp", "expected ftyp atom");

		let moov = read_atom(&mut reader)?;
		anyhow::ensure!(&moov[4..8] == b"moov", "expected moov atom");

		let mut init = ftyp;
		init.extend(&moov);

		// We're going to parse the moov box.
		// We have to read the moov box header to correctly advance the cursor for the mp4 crate.
		let mut moov_reader = io::Cursor::new(&moov);
		let moov_header = mp4::BoxHeader::read(&mut moov_reader)?;

		// Parse the moov box so we can detect the timescales for each track.
		let moov = mp4::MoovBox::read_box(&mut moov_reader, moov_header.size)?;

		let mut tracks = HashMap::new();

		// Create the init track
		let init_track = Self::create_init_track(init);
		tracks.insert("catalog".to_string(), init_track);

		// Create a map with the current segment for each track.
		// NOTE: We don't add the init track to this, since it's not part of the MP4.
		let mut sources = HashMap::new();

		for trak in &moov.traks {
			let track_id = trak.tkhd.track_id;
			anyhow::ensure!(track_id != 0xff, "track ID 0xff is reserved");

			let timescale = track_timescale(&moov, track_id);

			let segments = Producer::<Segment>::new();

			// Insert the subscribable track for consumerts.
			// The track_name is just the integer track ID.
			let track_name = track_id.to_string();
			tracks.insert(
				track_name,
				Track {
					segments: segments.subscribe(),
				},
			);

			// Store the track publisher in a map so we can update it later.
			let source = SourceTrack::new(segments, timescale);
			sources.insert(track_id, source);
		}

		let broadcast = Broadcast {
			tracks: Arc::new(tracks),
		};

		Ok(Self {
			reader,
			broadcast,
			tracks: sources,
		})
	}

	// Create an init track
	fn create_init_track(raw: Vec<u8>) -> Track {
		let mut fragments = Producer::new();
		let mut segments = Producer::new();

		fragments.push(raw.into());

		segments.push(Segment {
			sequence: VarInt::from_u32(0),   // first and only segment
			send_order: VarInt::from_u32(0), // highest priority
			expires: None,                   // never delete from the cache
			fragments: fragments.subscribe(),
		});

		Track {
			segments: segments.subscribe(),
		}
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		// The timestamp when the broadcast "started", so we can sleep to simulate a live stream.
		let start = tokio::time::Instant::now();

		// The ID of the last moof header.
		let mut track_id = None;

		loop {
			let atom = read_atom(&mut self.reader)?;

			let mut reader = io::Cursor::new(&atom);
			let header = mp4::BoxHeader::read(&mut reader)?;

			match header.name {
				mp4::BoxType::MoofBox => {
					let moof = mp4::MoofBox::read_box(&mut reader, header.size).context("failed to read MP4")?;

					// Process the moof.
					let fragment = SourceFragment::new(moof)?;

					// Get the track for this moof.
					let track = self.tracks.get_mut(&fragment.track).context("failed to find track")?;

					// Sleep until we should publish this sample.
					let timestamp = time::Duration::from_millis(1000 * fragment.timestamp / track.timescale);
					tokio::time::sleep_until(start + timestamp).await;

					// Save the track ID for the next iteration, which must be a mdat.
					anyhow::ensure!(track_id.is_none(), "multiple moof atoms");
					track_id.replace(fragment.track);

					// Publish the moof header, creating a new segment if it's a keyframe.
					track.header(atom, fragment).context("failed to publish moof")?;
				}
				mp4::BoxType::MdatBox => {
					// Get the track ID from the previous moof.
					let track_id = track_id.take().context("missing moof")?;
					let track = self.tracks.get_mut(&track_id).context("failed to find track")?;

					// Publish the mdat atom.
					track.data(atom).context("failed to publish mdat")?;
				}
				_ => {
					// Skip unknown atoms
				}
			}
		}
	}

	pub fn broadcast(&self) -> Broadcast {
		self.broadcast.clone()
	}
}

struct SourceTrack {
	// The track we're producing
	segments: Producer<Segment>,

	// The current segment's fragments
	fragments: Option<Producer<Fragment>>,

	// The number of units per second.
	timescale: u64,

	// The number of segments produced.
	sequence: u64,
}

impl SourceTrack {
	fn new(segments: Producer<Segment>, timescale: u64) -> Self {
		Self {
			segments,
			sequence: 0,
			fragments: None,
			timescale,
		}
	}

	pub fn header(&mut self, raw: Vec<u8>, fragment: SourceFragment) -> anyhow::Result<()> {
		if let Some(fragments) = self.fragments.as_mut() {
			if !fragment.keyframe {
				// Use the existing segment
				fragments.push(raw.into());
				return Ok(());
			}
		}

		// Otherwise make a new segment
		let now = time::Instant::now();

		// Compute the timestamp in milliseconds.
		// Overflows after 583 million years, so we're fine.
		let timestamp = fragment
			.timestamp(self.timescale)
			.as_millis()
			.try_into()
			.context("timestamp too large")?;

		// The send order is simple; newer timestamps are higher priority.
		// TODO give audio a boost?
		let send_order = VarInt::MAX
			.into_inner()
			.checked_sub(timestamp)
			.context("timestamp too large")?
			.try_into()
			.unwrap();

		// Delete segments after 10s.
		let expires = Some(now + time::Duration::from_secs(10));
		let sequence = self.sequence.try_into().context("sequence too large")?;

		self.sequence += 1;

		// Create a new segment, and save the fragments producer so we can push to it.
		let mut fragments = Producer::<Fragment>::new();
		self.segments.push(Segment {
			sequence,
			expires,
			send_order,
			fragments: fragments.subscribe(),
		});

		// Remove any segments older than 10s.
		// TODO This can only drain from the FRONT of the queue, so don't get clever with expirations.
		self.segments.drain(|segment| segment.expires.unwrap() < now);

		// Insert the raw atom into the segment.
		fragments.push(raw.into());

		// Save for the next iteration
		self.fragments = Some(fragments);

		Ok(())
	}

	pub fn data(&mut self, raw: Vec<u8>) -> anyhow::Result<()> {
		let fragments = self.fragments.as_mut().context("missing keyframe")?;
		fragments.push(raw.into());

		Ok(())
	}
}

struct SourceFragment {
	// The track for this fragment.
	track: u32,

	// The timestamp of the first sample in this fragment, in timescale units.
	timestamp: u64,

	// True if this fragment is a keyframe.
	keyframe: bool,
}

impl SourceFragment {
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

// Read a full MP4 atom into a vector.
fn read_atom<R: Read>(reader: &mut R) -> anyhow::Result<Vec<u8>> {
	// Read the 8 bytes for the size + type
	let mut buf = [0u8; 8];
	reader.read_exact(&mut buf)?;

	// Convert the first 4 bytes into the size.
	let size = u32::from_be_bytes(buf[0..4].try_into()?) as u64;
	//let typ = &buf[4..8].try_into().ok().unwrap();

	let mut raw = buf.to_vec();

	let mut limit = match size {
		// Runs until the end of the file.
		0 => reader.take(u64::MAX),

		// The next 8 bytes are the extended size to be used instead.
		1 => {
			reader.read_exact(&mut buf)?;
			let size_large = u64::from_be_bytes(buf);
			anyhow::ensure!(size_large >= 16, "impossible extended box size: {}", size_large);

			reader.take(size_large - 16)
		}

		2..=7 => {
			anyhow::bail!("impossible box size: {}", size)
		}

		// Otherwise read based on the size.
		size => reader.take(size - 8),
	};

	// Append to the vector and return it.
	limit.read_to_end(&mut raw)?;

	Ok(raw)
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

fn sample_timestamp(moof: &mp4::MoofBox) -> Option<u64> {
	Some(moof.trafs.first()?.tfdt.as_ref()?.base_media_decode_time)
}

/*
fn track_type(moov: &mp4::MoovBox, track_id: u32) -> mp4::TrackType {
	let trak = moov
		.traks
		.iter()
		.find(|trak| trak.tkhd.track_id == track_id)
		.expect("failed to find trak");

	mp4::TrackType::try_from(&trak.mdia.hdlr.handler_type).expect("unknown track type")
}
*/
