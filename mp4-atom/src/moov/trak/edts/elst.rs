use crate::*;

ext! {
    name: Elst,
    versions: [0, 1],
    flags: {}
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Elst {
    pub entries: Vec<ElstEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ElstEntry {
    pub segment_duration: u64,
    pub media_time: u64,
    pub media_rate: u16,
    pub media_rate_fraction: u16,
}

impl AtomExt for Elst {
    type Ext = ElstExt;

    const KIND_EXT: FourCC = FourCC::new(b"elst");

    fn decode_body_ext<B: Buf>(buf: &mut B, ext: ElstExt) -> Result<Self> {
        let entry_count = u32::decode(buf)?;

        let mut entries = Vec::new();
        for _ in 0..entry_count {
            let (segment_duration, media_time) = match ext.version {
                ElstVersion::V1 => (u64::decode(buf)?, u64::decode(buf)?),
                ElstVersion::V0 => (u32::decode(buf)? as u64, u32::decode(buf)? as u64),
            };

            let entry = ElstEntry {
                segment_duration,
                media_time,
                media_rate: u16::decode(buf)?,
                media_rate_fraction: u16::decode(buf)?,
            };
            entries.push(entry);
        }

        Ok(Elst { entries })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<ElstExt> {
        (self.entries.len() as u32).encode(buf)?;

        for entry in self.entries.iter() {
            entry.segment_duration.encode(buf)?;
            entry.media_time.encode(buf)?;
            entry.media_rate.encode(buf)?;
            entry.media_rate_fraction.encode(buf)?;
        }

        Ok(ElstVersion::V1.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elst32() {
        let expected = Elst {
            entries: vec![ElstEntry {
                segment_duration: 634634,
                media_time: 0,
                media_rate: 1,
                media_rate_fraction: 0,
            }],
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Elst::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_elst64() {
        let expected = Elst {
            entries: vec![ElstEntry {
                segment_duration: 634634,
                media_time: 0,
                media_rate: 1,
                media_rate_fraction: 0,
            }],
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Elst::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
