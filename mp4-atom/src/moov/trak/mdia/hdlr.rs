use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hdlr {
    pub handler: FourCC,
    pub name: String,
}

impl Default for Hdlr {
    fn default() -> Self {
        Hdlr {
            handler: FourCC::new(b"none"),
            name: String::new(),
        }
    }
}

impl AtomExt for Hdlr {
    type Ext = ();
    const KIND_EXT: FourCC = FourCC::new(b"hdlr");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        u32::decode(buf)?; // pre-defined
        let handler = FourCC::decode(buf)?;

        <[u8; 12]>::decode(buf)?; // reserved

        let name = String::decode(buf)?;

        // Skip any trailing padding
        //buf.advance(buf.remaining());

        Ok(Hdlr { handler, name })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        0u32.encode(buf)?; // pre-defined
        self.handler.encode(buf)?;

        // 12 bytes reserved
        [0u8; 12].encode(buf)?;

        self.name.as_str().encode(buf)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hdlr() {
        let expected = Hdlr {
            handler: FourCC::new(b"vide"),
            name: String::from("VideoHandler"),
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Hdlr::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_hdlr_empty() {
        let expected = Hdlr {
            handler: FourCC::new(b"vide"),
            name: String::new(),
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Hdlr::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
