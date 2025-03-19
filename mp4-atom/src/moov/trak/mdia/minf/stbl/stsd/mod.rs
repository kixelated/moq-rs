mod av01;
mod h264;
mod hevc;
mod mp4a;
mod tx3g;
mod visual;
mod vp9;

pub use av01::*;
pub use h264::*;
pub use hevc::*;
pub use mp4a::*;
pub use tx3g::*;
pub use visual::*;
pub use vp9::*;

use crate::*;
use derive_more::From;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stsd {
    pub codecs: Vec<Codec>,
}

/// Called a "sample entry" in the ISOBMFF specification.
#[derive(Debug, Clone, PartialEq, Eq, From)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Codec {
    // H264
    Avc1(Avc1),

    // HEVC: SPS/PPS/VPS is inline
    Hev1(Hev1),

    // HEVC: SPS/PPS/VPS is in a separate atom
    Hvc1(Hvc1),

    // VP8
    Vp08(Vp08),

    // VP9
    Vp09(Vp09),

    // AV1
    Av01(Av01),

    // AAC
    Mp4a(Mp4a),

    // Text
    Tx3g(Tx3g),

    // Unknown
    Unknown(FourCC),
}

impl Into<Avc1> for Codec {
    fn into(self) -> Avc1 {
        match self {
            Codec::Avc1(atom) => atom,
            _ => panic!("invalid codec type"),
        }
    }
}

impl Into<Hev1> for Codec {
    fn into(self) -> Hev1 {
        match self {
            Codec::Hev1(atom) => atom,
            _ => panic!("invalid codec type"),
        }
    }
}

impl Into<Hvc1> for Codec {
    fn into(self) -> Hvc1 {
        match self {
            Codec::Hvc1(atom) => atom,
            _ => panic!("invalid codec type"),
        }
    }
}

impl Into<Vp08> for Codec {
    fn into(self) -> Vp08 {
        match self {
            Codec::Vp08(atom) => atom,
            _ => panic!("invalid codec type"),
        }
    }
}

impl Into<Vp09> for Codec {
    fn into(self) -> Vp09 {
        match self {
            Codec::Vp09(atom) => atom,
            _ => panic!("invalid codec type"),
        }
    }
}

impl Into<Mp4a> for Codec {
    fn into(self) -> Mp4a {
        match self {
            Codec::Mp4a(atom) => atom,
            _ => panic!("invalid codec type"),
        }
    }
}

impl Into<Tx3g> for Codec {
    fn into(self) -> Tx3g {
        match self {
            Codec::Tx3g(atom) => atom,
            _ => panic!("invalid codec type"),
        }
    }
}

impl Into<Av01> for Codec {
    fn into(self) -> Av01 {
        match self {
            Codec::Av01(atom) => atom,
            _ => panic!("invalid codec type"),
        }
    }
}

impl Decode for Codec {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let atom = Any::decode(buf)?;
        Ok(match atom {
            Any::Avc1(atom) => atom.into(),
            Any::Hev1(atom) => atom.into(),
            Any::Hvc1(atom) => atom.into(),
            Any::Vp08(atom) => atom.into(),
            Any::Vp09(atom) => atom.into(),
            Any::Mp4a(atom) => atom.into(),
            Any::Tx3g(atom) => atom.into(),
            Any::Av01(atom) => atom.into(),
            Any::Unknown(kind, _) => Self::Unknown(kind),
            _ => return Err(Error::UnexpectedBox(atom.kind())),
        })
    }
}

impl Encode for Codec {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        match self {
            Self::Unknown(kind) => kind.encode(buf),
            Self::Avc1(atom) => atom.encode(buf),
            Self::Hev1(atom) => atom.encode(buf),
            Self::Hvc1(atom) => atom.encode(buf),
            Self::Vp08(atom) => atom.encode(buf),
            Self::Vp09(atom) => atom.encode(buf),
            Self::Mp4a(atom) => atom.encode(buf),
            Self::Tx3g(atom) => atom.encode(buf),
            Self::Av01(atom) => atom.encode(buf),
        }
    }
}

impl AtomExt for Stsd {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"stsd");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let codec_count = u32::decode(buf)?;
        let mut codecs = Vec::new();

        for _ in 0..codec_count {
            let codec = Codec::decode(buf)?;
            codecs.push(codec);
        }

        Ok(Stsd { codecs })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        (self.codecs.len() as u32).encode(buf)?;
        for codec in &self.codecs {
            codec.encode(buf)?;
        }

        Ok(())
    }
}
