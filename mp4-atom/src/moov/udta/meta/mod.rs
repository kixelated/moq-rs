mod ilst;
pub use ilst::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Meta {
    Mdir { ilst: Option<Ilst> },
    Unknown { hdlr: Hdlr, data: Vec<u8> },
}

impl Default for Meta {
    fn default() -> Self {
        Self::Mdir { ilst: None }
    }
}

const MDIR: FourCC = FourCC::new(b"mdir");

impl AtomExt for Meta {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"meta");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let hdlr = Hdlr::decode(buf)?;

        match hdlr.handler {
            MDIR => {
                let ilst = Ilst::decode_maybe(buf)?;
                Ok(Meta::Mdir { ilst })
            }
            _ => {
                let data = Vec::<u8>::decode(buf)?;
                Ok(Meta::Unknown { hdlr, data })
            }
        }
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        match self {
            Self::Mdir { ilst } => {
                Hdlr {
                    handler: MDIR,
                    ..Default::default()
                }
                .encode(buf)?;
                ilst.encode(buf)?;
            }
            Self::Unknown { hdlr, data } => {
                hdlr.encode(buf)?;
                data.encode(buf)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_mdir_empty() {
        let expected = Meta::Mdir { ilst: None };

        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let output = Meta::decode(&mut buf).unwrap();
        assert_eq!(output, expected);
    }

    #[test]
    fn test_meta_mdir() {
        let expected = Meta::Mdir {
            ilst: Some(Ilst::default()),
        };

        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let output = Meta::decode(&mut buf).unwrap();
        assert_eq!(output, expected);
    }
}
