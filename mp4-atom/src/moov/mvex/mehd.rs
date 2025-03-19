use crate::*;

ext! {
    name: Mehd,
    versions: [0,1],
    flags: {}
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mehd {
    pub fragment_duration: u64,
}

impl AtomExt for Mehd {
    const KIND_EXT: FourCC = FourCC::new(b"mehd");

    type Ext = MehdExt;

    fn decode_body_ext<B: Buf>(buf: &mut B, ext: MehdExt) -> Result<Self> {
        let fragment_duration = match ext.version {
            MehdVersion::V1 => u64::decode(buf)?,
            MehdVersion::V0 => u32::decode(buf)? as u64,
        };

        Ok(Mehd { fragment_duration })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<MehdExt> {
        self.fragment_duration.encode(buf)?;
        Ok(MehdVersion::V1.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mehd32() {
        let expected = Mehd {
            fragment_duration: 32,
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Mehd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_mehd64() {
        let expected = Mehd {
            fragment_duration: 30439936,
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Mehd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
