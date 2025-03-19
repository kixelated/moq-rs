use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Smhd {
    pub balance: FixedPoint<i8>,
}

impl AtomExt for Smhd {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"smhd");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let balance = FixedPoint::decode(buf)?;
        u16::decode(buf)?; // reserved?

        Ok(Smhd { balance })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.balance.encode(buf)?;
        0u16.encode(buf)?; // reserved

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smhd() {
        let expected = Smhd {
            balance: FixedPoint::from(-1),
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Smhd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
