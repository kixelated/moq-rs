use crate::*;

ext! {
    name: Url,
    versions: [0],
    flags: {
        self_contained = 1,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Url {
    pub location: String,
}

impl AtomExt for Url {
    type Ext = UrlExt;

    const KIND_EXT: FourCC = FourCC::new(b"url ");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: UrlExt) -> Result<Self> {
        let location = match buf.has_remaining() {
            true => String::decode(buf)?,
            false => "".to_string(),
        };

        Ok(Url { location })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<UrlExt> {
        if !self.location.is_empty() {
            self.location.as_str().encode(buf)?;
        }

        Ok(UrlExt {
            // TODO what does this do?
            self_contained: true,
            ..Default::default()
        })
    }
}
