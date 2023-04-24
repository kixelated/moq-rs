use std::{io,fs,time};
use io::Read;

use mp4;
use anyhow;

use mp4::ReadBox;

pub struct Source {
    reader: io::BufReader<fs::File>,
    pending: Option<Fragment>,

    start: time::Instant,
    timescale: Option<u64>,
}

pub struct Fragment {
    pub typ: mp4::BoxType,
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub timestamp: Option<u64>, // only used to simulate a live stream
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
            timescale: None,
        })
    }

    pub fn next(&mut self) -> anyhow::Result<Option<Fragment>> {
        if self.pending.is_none() {
            self.pending = Some(self.next_inner()?);
        };

        if self.timeout().is_some() {
            return Ok(None)
        }

        let pending = self.pending.take();
        Ok(pending)
    }

    fn next_inner(&mut self) -> anyhow::Result<Fragment> {
        // Read the next full atom.
        let atom = read_box(&mut self.reader)?;
        let mut timestamp = None;
        let mut keyframe = false;

        // Before we return it, let's do some simple parsing.
        let mut reader = io::Cursor::new(&atom);
        let header = mp4::BoxHeader::read(&mut reader)?;

        match header.name {
            mp4::BoxType::MoovBox => {
                // We need to parse the moov to get the timescale.
                let moov = mp4::MoovBox::read_box(&mut reader, header.size)?;
                self.timescale = Some(moov.traks[0].mdia.mdhd.timescale.into());
            },
            mp4::BoxType::MoofBox => {
                let moof = mp4::MoofBox::read_box(&mut reader, header.size)?;

                keyframe = has_keyframe(&moof);
                timestamp = first_timestamp(&moof);
            }
            _ => {},
        }

        Ok(Fragment {
            typ: header.name,
            data: atom,
            keyframe,
            timestamp,
        })
    }

    // Simulate a live stream by sleeping until the next timestamp in the media.
    pub fn timeout(&self) -> Option<time::Duration> {
        let timestamp = self.pending.as_ref()?.timestamp?;
        let timescale = self.timescale?;

        let delay = time::Duration::from_millis(1000 * timestamp / timescale);
        let elapsed = self.start.elapsed();

        delay.checked_sub(elapsed)
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

            if keyframe && !non_sync {
                return true
            }
        }
    }

    false
}

fn first_timestamp(moof: &mp4::MoofBox) -> Option<u64> {
    Some(moof.trafs.first()?.tfdt.as_ref()?.base_media_decode_time)
}
