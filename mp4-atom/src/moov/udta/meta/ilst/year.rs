use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Year(pub String);

impl Atom for Year {
    const KIND: FourCC = FourCC::new(b"day ");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self(String::decode(buf)?))
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.0.as_str().encode(buf)
    }
}
