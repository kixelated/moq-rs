use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Name(pub String);

impl Atom for Name {
    const KIND: FourCC = FourCC::new(b"name");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Name(String::decode(buf)?))
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.0.as_str().encode(buf)
    }
}
