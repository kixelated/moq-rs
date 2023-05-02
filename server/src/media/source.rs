use io::Read;
use std::collections::VecDeque;
use std::{fs, io, time};

use std::io::Write;

use anyhow;
use mp4;

use mp4::{ReadBox, WriteBox};

pub struct Source {
    // We read the file once, in order, and don't seek backwards.
    reader: io::BufReader<fs::File>,

    // Any fragments parsed and ready to be returned by next().
    fragments: VecDeque<Fragment>,

    // The timestamp when the broadcast "started", so we can sleep to simulate a live stream.
    start: time::Instant,

    // The raw ftyp box, which we need duplicate for each track, but we don't know how many tracks exist yet.
    ftyp: Vec<u8>,

    // The parsed moov box, so we can look up track information later.
    moov: Option<mp4::MoovBox>,
}

pub struct Fragment {
    // The track ID for the fragment.
    pub track: u32,

    // The type of the fragment.
    pub typ: mp4::BoxType,

    // The data of the fragment.
    pub data: Vec<u8>,

    // Whether this fragment is a keyframe.
    pub keyframe: bool,

    // The timestamp of the fragment, in milliseconds, to simulate a live stream.
    pub timestamp: Option<u64>,
}

impl Source {
    pub fn new(path: &str) -> io::Result<Self> {
        let f = fs::File::open(path)?;
        let reader = io::BufReader::new(f);
        let start = time::Instant::now();

        Ok(Self {
            reader,
            start,
            fragments: VecDeque::new(),
            ftyp: Vec::new(),
            moov: None,
        })
    }

    pub fn next(&mut self) -> anyhow::Result<Option<Fragment>> {
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
            // Read the next full atom.
            let atom = read_box(&mut self.reader)?;

            // Before we return it, let's do some simple parsing.
            let mut reader = io::Cursor::new(&atom);
            let header = mp4::BoxHeader::read(&mut reader)?;

            match header.name {
                mp4::BoxType::FtypBox => {
                    // Don't return anything until we know the total number of tracks.
                    // To be honest, I didn't expect the borrow checker to allow this, but it does!
                    self.ftyp = atom;
                }
                mp4::BoxType::MoovBox => {
                    // We need to split the moov based on the tracks.
                    let moov = mp4::MoovBox::read_box(&mut reader, header.size)?;

                    for trak in &moov.traks {
                        let track_id = trak.tkhd.track_id;

                        // Push the styp atom for each track.
                        self.fragments.push_back(Fragment {
                            track: track_id,
                            typ: mp4::BoxType::FtypBox,
                            data: self.ftyp.clone(),
                            keyframe: false,
                            timestamp: None,
                        });

                        // Unfortunately, we need to create a brand new moov atom for each track.
                        // We remove every box for other track IDs.
                        let mut toov = moov.clone();
                        toov.traks.retain(|t| t.tkhd.track_id == track_id);
                        toov.mvex
                            .as_mut()
                            .expect("missing mvex")
                            .trexs
                            .retain(|f| f.track_id == track_id);

                        // Marshal the box.
                        let mut toov_data = Vec::new();
                        toov.write_box(&mut toov_data)?;

                        let mut file = std::fs::File::create(format!("track{}.mp4", track_id))?;
                        file.write_all(toov_data.as_slice());

                        self.fragments.push_back(Fragment {
                            track: track_id,
                            typ: mp4::BoxType::MoovBox,
                            data: toov_data,
                            keyframe: false,
                            timestamp: None,
                        });
                    }

                    self.moov = Some(moov);
                }
                mp4::BoxType::MoofBox => {
                    let moof = mp4::MoofBox::read_box(&mut reader, header.size)?;

                    if moof.trafs.len() != 1 {
                        // We can't split the mdat atom, so this is impossible to support
                        anyhow::bail!("multiple tracks per moof atom")
                    }

                    self.fragments.push_back(Fragment {
                        track: moof.trafs[0].tfhd.track_id,
                        typ: mp4::BoxType::MoofBox,
                        data: atom,
                        keyframe: has_keyframe(&moof),
                        timestamp: first_timestamp(&moof),
                    })
                }
                mp4::BoxType::MdatBox => {
                    let moof = self.fragments.back().expect("no atom before mdat");
                    assert!(moof.typ == mp4::BoxType::MoofBox, "no moof before mdat");

                    self.fragments.push_back(Fragment {
                        track: moof.track,
                        typ: mp4::BoxType::MoofBox,
                        data: atom,
                        keyframe: false,
                        timestamp: None,
                    });

                    // We have some media data, return so we can start sending it.
                    return Ok(());
                }
                _ => anyhow::bail!("unknown top-level atom: {:?}", header.name),
            }
        }
    }

    // Simulate a live stream by sleeping until the next timestamp in the media.
    pub fn timeout(&self) -> Option<time::Duration> {
        let next = self.fragments.front()?;
        let timestamp = next.timestamp?;

        // Find the timescale for the track.
        let track = self
            .moov
            .as_ref()?
            .traks
            .iter()
            .find(|t| t.tkhd.track_id == next.track)?;
        let timescale = track.mdia.mdhd.timescale as u64;

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
            anyhow::ensure!(
                size_large >= 16,
                "impossible extended box size: {}",
                size_large
            );

            reader.take(size_large - 16)
        }

        2..=7 => {
            anyhow::bail!("impossible box size: {}", size)
        }

        // Otherwise read based on the size.
        size => reader.take(size - 8),
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
                return true;
            }
        }
    }

    false
}

fn first_timestamp(moof: &mp4::MoofBox) -> Option<u64> {
    Some(moof.trafs.first()?.tfdt.as_ref()?.base_media_decode_time)
}
