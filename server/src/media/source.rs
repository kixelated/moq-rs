use std::collections::VecDeque;
use std::io::Read;
use std::{fs, io, time};

use anyhow;

use mp4;
use mp4::ReadBox;

pub struct Source {
	// We read the file once, in order, and don't seek backwards.
	reader: io::BufReader<fs::File>,

	// The timestamp when the broadcast "started", so we can sleep to simulate a live stream.
	start: time::Instant,

	// The initialization payload; ftyp + moov boxes.
	pub init: Vec<u8>,

	// The parsed moov box.
	moov: mp4::MoovBox,

	// Any fragments parsed and ready to be returned by next().
	fragments: VecDeque<Fragment>,
}

pub struct Fragment {
	// The track ID for the fragment.
	pub track_id: u32,

	// The data of the fragment.
	pub data: Vec<u8>,

	// Whether this fragment is a keyframe.
	pub keyframe: bool,

	// The number of samples that make up a second (ex. ms = 1000)
	pub timescale: u64,

	// The timestamp of the fragment, in timescale units, to simulate a live stream.
	pub timestamp: u64,
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

		Ok(Self {
			reader,
			start,
			init,
			moov,
			fragments: VecDeque::new(),
		})
	}

	pub fn fragment(&mut self) -> anyhow::Result<Option<Fragment>> {
		if self.fragments.is_empty() {
			self.parse()?;
		};

		if self.timeout().is_some() {
			return Ok(None);
		}

		Ok(self.fragments.pop_front())
	}

	fn parse(&mut self) -> anyhow::Result<()> {
		loop {
			let atom = read_atom(&mut self.reader)?;

			let mut reader = io::Cursor::new(&atom);
			let header = mp4::BoxHeader::read(&mut reader)?;

			match header.name {
				mp4::BoxType::FtypBox | mp4::BoxType::MoovBox => {
					anyhow::bail!("must call init first")
				}
				mp4::BoxType::MoofBox => {
					let moof = mp4::MoofBox::read_box(&mut reader, header.size)?;

					if moof.trafs.len() != 1 {
						// We can't split the mdat atom, so this is impossible to support
						anyhow::bail!("multiple tracks per moof atom")
					}

					let track_id = moof.trafs[0].tfhd.track_id;
					let timestamp = sample_timestamp(&moof).expect("couldn't find timestamp");

					// Detect if this is a keyframe.
					let keyframe = sample_keyframe(&moof);

					let timescale = track_timescale(&self.moov, track_id);

					self.fragments.push_back(Fragment {
						track_id,
						data: atom,
						keyframe,
						timescale,
						timestamp,
					})
				}
				mp4::BoxType::MdatBox => {
					let moof = self.fragments.back().expect("no atom before mdat");

					self.fragments.push_back(Fragment {
						track_id: moof.track_id,
						data: atom,
						keyframe: false,
						timescale: moof.timescale,
						timestamp: moof.timestamp,
					});

					// We have some media data, return so we can start sending it.
					return Ok(());
				}
				_ => {
					// Skip unknown atoms
				}
			}
		}
	}

	// Simulate a live stream by sleeping until the next timestamp in the media.
	pub fn timeout(&self) -> Option<time::Duration> {
		let next = self.fragments.front()?;

		let delay = time::Duration::from_millis(1000 * next.timestamp / next.timescale);
		let elapsed = self.start.elapsed();

		delay.checked_sub(elapsed)
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
