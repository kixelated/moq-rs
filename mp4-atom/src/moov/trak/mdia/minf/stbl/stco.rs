use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stco {
    pub entries: Vec<u32>,
}

impl AtomExt for Stco {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"stco");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let count = u32::decode(buf)?;
        let mut entries = Vec::new();

        for _ in 0..count {
            let chunk_offset = u32::decode(buf)?;
            entries.push(chunk_offset);
        }

        Ok(Stco { entries })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        (self.entries.len() as u32).encode(buf)?;
        for chunk_offset in self.entries.iter() {
            (chunk_offset).encode(buf)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stco() {
        let expected = Stco {
            entries: vec![267, 1970, 2535, 2803, 11843, 22223, 33584],
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Stco::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
