use std::{io,fs,time};
use io::Read;

use mp4;
use anyhow;

use mp4::ReadBox;

pub struct Source {
    reader: io::BufReader<fs::File>,
    start: time::Instant,

    pending: Option<Fragment>,
    sequence: u64,
}

pub struct Fragment {
    pub data: Vec<u8>,
    pub segment_id: u64,
    pub timestamp: u64,
}

impl Source {
    pub fn new(path: &str) -> io::Result<Self> {
        let f = fs::File::open(path)?;
        let reader = io::BufReader::new(f);
        let start = time::Instant::now();

        Ok(Self{
            reader,
            start,
            pending: None,
            sequence: 0,
        })
    }

    pub fn next(&mut self) -> anyhow::Result<Option<Fragment>> {
        let pending = match self.pending.take() {
            Some(f) => f,
            None => self.next_inner()?,
        };

        if pending.timestamp > 0 && pending.timestamp < self.start.elapsed().as_millis() as u64 {
            self.pending = Some(pending);
            return Ok(None)
        }

        Ok(Some(pending))
    }

    fn next_inner(&mut self) -> anyhow::Result<Fragment> {
        // Read the next full atom.
        let atom = read_box(&mut self.reader)?;
        let mut timestamp = 0;

        // Before we return it, let's do some simple parsing.
        let mut reader = io::Cursor::new(&atom);
        let header = mp4::BoxHeader::read(&mut reader)?;


        match header.name {
            mp4::BoxType::MoofBox => {
                let moof = mp4::MoofBox::read_box(&mut reader, header.size)?;

                if has_keyframe(&moof) {
                    self.sequence += 1
                }

                timestamp = first_timestamp(&moof);
            }
            _ => (),
        }

        Ok(Fragment {
            data: atom,
            segment_id: self.sequence,
            timestamp: timestamp,
        })
    }
}

// Read a full MP4 atom into a vector.
fn read_box<R: io::Read>(reader: &mut R) -> anyhow::Result<Vec<u8>> {
    // Read the 8 bytes for the size + type
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;

    // Convert the first 4 bytes into the size.
    let size = u32::from_be_bytes(buf[0..4].try_into()?) as u64;
    let mut out = buf.to_vec();

    let mut limit = match size {
        // Runs until the end of the file.
        0 => reader.take(u64::MAX),

        // The next 8 bytes are the extended size to be used instead.
        1 => {
            reader.read_exact(&mut buf)?;
            let size_large = u64::from_be_bytes(buf);
            anyhow::ensure!(size_large >= 16, "impossible extended box size: {}", size_large);

            reader.take(size_large - 16)
        },

        2..=7 => {
            anyhow::bail!("impossible box size: {}", size)
        }

        // Otherwise read based on the size.
        size => reader.take(size - 8)
    };

    // Append to the vector and return it.
    limit.read_to_end(&mut out)?;

    Ok(out)
}

fn has_keyframe(moof: &mp4::MoofBox) -> bool {
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

            if keyframe && non_sync {
                return true
            }
        }
    }

    false
}

fn first_timestamp(moof: &mp4::MoofBox) -> u64 {
    let traf = match moof.trafs.first() {
        Some(t) => t,
        None => return 0,
    };

    let tfdt = match &traf.tfdt {
        Some(t) => t,
        None => return 0,
    };

    tfdt.base_media_decode_time
}
