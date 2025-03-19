use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Skip {
    pub zeroed: Zeroed,
}

impl Atom for Skip {
    const KIND: FourCC = FourCC::new(b"skip");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            zeroed: Zeroed::decode(buf)?,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.zeroed.encode(buf)
    }
}
