use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stts {
    pub entries: Vec<SttsEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SttsEntry {
    pub sample_count: u32,
    pub sample_delta: u32,
}

impl AtomExt for Stts {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"stts");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let entry_count = u32::decode(buf)?;
        let mut entries = Vec::new();
        for _ in 0..entry_count {
            let entry = SttsEntry {
                sample_count: u32::decode(buf)?,
                sample_delta: u32::decode(buf)?,
            };
            entries.push(entry);
        }

        Ok(Stts { entries })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        (self.entries.len() as u32).encode(buf)?;
        for entry in self.entries.iter() {
            entry.sample_count.encode(buf)?;
            entry.sample_delta.encode(buf)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stts() {
        let expected = Stts {
            entries: vec![
                SttsEntry {
                    sample_count: 29726,
                    sample_delta: 1024,
                },
                SttsEntry {
                    sample_count: 1,
                    sample_delta: 512,
                },
            ],
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Stts::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
