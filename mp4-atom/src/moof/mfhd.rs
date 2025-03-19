use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mfhd {
    pub sequence_number: u32,
}

impl Default for Mfhd {
    fn default() -> Self {
        Mfhd { sequence_number: 1 }
    }
}

impl AtomExt for Mfhd {
    type Ext = ();
    const KIND_EXT: FourCC = FourCC::new(b"mfhd");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        Ok(Mfhd {
            sequence_number: u32::decode(buf)?,
        })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.sequence_number.encode(buf)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mfhd() {
        let expected = Mfhd { sequence_number: 1 };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Mfhd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
