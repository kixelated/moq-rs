use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ctts {
    pub entries: Vec<CttsEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CttsEntry {
    pub sample_count: u32,
    pub sample_offset: i32,
}

impl AtomExt for Ctts {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"ctts");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let entry_count = u32::decode(buf)?;

        let mut entries = Vec::new();
        for _ in 0..entry_count {
            let entry = CttsEntry {
                sample_count: u32::decode(buf)?,
                sample_offset: i32::decode(buf)?,
            };
            entries.push(entry);
        }

        Ok(Ctts { entries })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        (self.entries.len() as u32).encode(buf)?;
        for entry in self.entries.iter() {
            (entry.sample_count).encode(buf)?;
            (entry.sample_offset).encode(buf)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ctts() {
        let expected = Ctts {
            entries: vec![
                CttsEntry {
                    sample_count: 1,
                    sample_offset: 200,
                },
                CttsEntry {
                    sample_count: 2,
                    sample_offset: -100,
                },
            ],
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Ctts::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
