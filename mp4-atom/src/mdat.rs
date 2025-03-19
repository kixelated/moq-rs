use crate::*;

/// A media data atom.
///
/// I would not recommend using this for large files, as it requires the entire file is loaded into memory.
/// Instead, use [ReadFrom] to read the [Header] first followed by the mdat data.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mdat {
    pub data: Vec<u8>,
}

impl Atom for Mdat {
    const KIND: FourCC = FourCC::new(b"mdat");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Mdat {
            data: Vec::decode(buf)?,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.data.encode(buf)
    }
}
