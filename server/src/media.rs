use std::{io,fs};

use mp4;
use anyhow;
use bytes;

use mp4::ReadBox;

pub struct Source {
    pub segments: Vec<Segment>,
}

impl Source {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let f = fs::read(path)?;
        let mut bytes = bytes::Bytes::from(f);

        let mut segments = Vec::new();
        let mut current = Segment::new();

        while bytes.len() > 0 {
            // NOTE: Cloning is cheap, since the underlying bytes are reference counted.
            let mut reader = io::Cursor::new(bytes.clone());

            let header = mp4::BoxHeader::read(&mut reader)?;
            let size: usize = header.size as usize;

            assert!(size > 0, "empty box");

            let frag = bytes.split_to(size);
            let fragment = Fragment{ bytes: frag };

            match header.name {
                /*
                mp4::BoxType::FtypBox => {
                }
                mp4::BoxType::MoovBox => {
                    moov = mp4::MoovBox::read_box(&mut reader, size)?
                }
                mp4::BoxType::EmsgBox => {
                    let emsg = mp4::EmsgBox::read_box(&mut reader, size)?;
                    emsgs.push(emsg);
                }
                mp4::BoxType::MdatBox => {
                    mp4::skip_box(&mut reader, size)?;
                }
                */
                mp4::BoxType::MoofBox => {
                    let moof = mp4::MoofBox::read_box(&mut reader, header.size)?;
                    if has_keyframe(moof) {
                        segments.push(current);
                        current = Segment::new();
                    }
                }
                _ => (),
            }

            current.fragments.push(fragment);
        }

        segments.push(current);

        Ok(Self { segments })
    }
}

fn has_keyframe(moof: mp4::MoofBox) -> bool {
    for traf in moof.trafs {
        // TODO trak default flags if this is None
        let default_flags = traf.tfhd.default_sample_flags.unwrap_or_default();
        let trun = traf.trun.expect("missing trun box");

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

pub struct Segment {
    pub fragments: Vec<Fragment>,
}

impl Segment {
    fn new() -> Self {
        Segment { fragments: Vec::new() }
    }
}

pub struct Fragment {
    pub bytes: bytes::Bytes,
}
