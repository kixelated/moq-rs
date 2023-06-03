use std::io::Read;
use std::sync::Mutex;
use std::{fs, io, time};

use anyhow;

use mp4;
use mp4::ReadBox;

use super::{Broadcast, Fragment, Segment, Track};

pub struct Source {
	// We read the file once, in order, and don't seek backwards.
	reader: io::BufReader<fs::File>,

	// The timestamp when the broadcast "started", so we can sleep to simulate a live stream.
	start: time::Instant,

	// The parsed moov box.
	moov: mp4::MoovBox,

	// The broadcast we're producing
	broadcast: Arc<Mutex<Broadcast>>,

	// The timestamp of the last fragment we pushed.
	timestamp: time::Duration,
}

impl Source {
	pub fn new(path: &str) -> anyhow::Result<Self> {
		let f = fs::File::open(path)?;
		let mut reader = io::BufReader::new(f);
		let start = time::Instant::now();

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

		// Create a broadcast.
		let broadcast = Broadcast::new();

		// Create a catalog track with the full init segment.
		// TODO the track ID is 0xff for now, but we should probably use a better way to signal catalog tracks.
		let segment = Segment::new();
		segment.add_fragment(init);

		let track = Track::new();
		track.add_segment(segment);

		broadcast.add_track(0xff, track);

		// Add the top level tracks.
		for track in &moov.traks {
			let track_id = track.tkhd.track_id;
			anyhow::ensure!(track_id != 0xff, "track ID 0xff is reserved");

			broadcast.add_track(track.tkhd.track_id, Track::new());
		}

		Ok(Self {
			reader,
			start,
			moov,
			broadcast: Arc::new(Mutex::new(broadcast)),
			timestamp: 0,
		})
	}

	pub fn poll(&mut self) -> anyhow::Result<Option<time::Duration>> {
		loop {
			let elapsed = self.start.elapsed();

			let timeout = self.timestamp.checked_sub(elapsed);
			if timeout.is_some() {
				return Ok(timeout);
			}

			self.parse()?;
		}
	}

	// Parse the next mdat atom and add it to the broadcast.
	pub fn parse(&mut self) -> anyhow::Result<()> {
		// The current segment for the track.
		let segment = None;

		loop {
			let atom = read_atom(&mut self.reader)?;

			let mut reader = io::Cursor::new(&atom);
			let header = mp4::BoxHeader::read(&mut reader)?;

			match header.name {
				mp4::BoxType::MoofBox => {
					let moof = mp4::MoofBox::read_box(&mut reader, header.size)?;

					if moof.trafs.len() != 1 {
						// We can't split the mdat atom, so this is impossible to support
						anyhow::bail!("multiple tracks per moof atom")
					}

					let track_id = moof.trafs[0].tfhd.track_id;
					let track = self.broadcast.get_track(track_id);

					// Parse the moof to get some timing information to sleep.
					let timestamp = sample_timestamp(&moof).expect("couldn't find timestamp");
					let timescale = track_timescale(&self.moov, track_id);

					// Update the timestamp so we'll sleep before parsing the next fragment.
					self.timestamp = time::Duration::from_millis(1000 * timestamp / timescale);

					// Detect if we should start a new segment.
					let keyframe = sample_keyframe(&moof);

					let last = if keyframe {
						track.add_segment(Segment::new())
					} else {
						track.last_segment().expect("missing keyframe at start of track")
					};

					last.push_fragment(atom);

					segment = Some(last)
				}
				mp4::BoxType::MdatBox => {
					segment.expect("missing moof before mdat").push_fragment(atom);
					return Ok(());
				}
				_ => {
					// Skip unknown atoms
				}
			}
		}
	}
}

// Read a full MP4 atom into a vector.
pub fn read_atom<R: Read>(reader: &mut R) -> anyhow::Result<Vec<u8>> {
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
