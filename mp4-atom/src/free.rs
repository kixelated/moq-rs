use crate::*;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Free {
    pub zeroed: Zeroed,
}

impl Atom for Free {
    const KIND: FourCC = FourCC::new(b"free");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Free {
            zeroed: Zeroed::decode(buf)?,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.zeroed.encode(buf)
    }
}
