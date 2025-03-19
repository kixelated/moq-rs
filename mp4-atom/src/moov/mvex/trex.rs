use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trex {
    pub track_id: u32,
    pub default_sample_description_index: u32,
    pub default_sample_duration: u32,
    pub default_sample_size: u32,
    pub default_sample_flags: u32,
}

impl AtomExt for Trex {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"trex");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        Ok(Trex {
            track_id: u32::decode(buf)?,
            default_sample_description_index: u32::decode(buf)?,
            default_sample_duration: u32::decode(buf)?,
            default_sample_size: u32::decode(buf)?,
            default_sample_flags: u32::decode(buf)?,
        })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.track_id.encode(buf)?;
        self.default_sample_description_index.encode(buf)?;
        self.default_sample_duration.encode(buf)?;
        self.default_sample_size.encode(buf)?;
        self.default_sample_flags.encode(buf)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trex() {
        let expected = Trex {
            track_id: 1,
            default_sample_description_index: 1,
            default_sample_duration: 1000,
            default_sample_size: 0,
            default_sample_flags: 65536,
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Trex::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
