mod url;
pub use url::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dref {
    pub urls: Vec<Url>,
}

impl AtomExt for Dref {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"dref");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let entry_count = u32::decode(buf)?;
        let mut urls = Vec::new();

        for _ in 0..entry_count {
            let url = Url::decode(buf)?;
            urls.push(url);
        }

        Ok(Dref { urls })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        (self.urls.len() as u32).encode(buf)?;

        for url in &self.urls {
            url.encode(buf)?;
        }

        Ok(())
    }
}
