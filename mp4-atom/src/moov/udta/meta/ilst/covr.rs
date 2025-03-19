use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Covr(pub Vec<u8>);

impl Atom for Covr {
    const KIND: FourCC = FourCC::new(b"covr");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Covr(Vec::decode(buf)?))
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.0.encode(buf)
    }
}
