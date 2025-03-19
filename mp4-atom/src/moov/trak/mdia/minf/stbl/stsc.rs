use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stsc {
    pub entries: Vec<StscEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StscEntry {
    pub first_chunk: u32,
    pub samples_per_chunk: u32,
    pub sample_description_index: u32,
}

impl AtomExt for Stsc {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"stsc");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let entry_count = u32::decode(buf)?;

        let mut entries = Vec::new();
        for _ in 0..entry_count {
            let entry = StscEntry {
                first_chunk: u32::decode(buf)?,
                samples_per_chunk: u32::decode(buf)?,
                sample_description_index: u32::decode(buf)?,
            };
            entries.push(entry);
        }

        Ok(Stsc { entries })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        (self.entries.len() as u32).encode(buf)?;
        for entry in self.entries.iter() {
            entry.first_chunk.encode(buf)?;
            entry.samples_per_chunk.encode(buf)?;
            entry.sample_description_index.encode(buf)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stsc() {
        let expected = Stsc {
            entries: vec![
                StscEntry {
                    first_chunk: 1,
                    samples_per_chunk: 1,
                    sample_description_index: 1,
                },
                StscEntry {
                    first_chunk: 19026,
                    samples_per_chunk: 14,
                    sample_description_index: 1,
                },
            ],
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Stsc::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
