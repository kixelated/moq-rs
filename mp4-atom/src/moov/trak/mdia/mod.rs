mod hdlr;
mod mdhd;
mod minf;

pub use hdlr::*;
pub use mdhd::*;
pub use minf::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mdia {
    pub mdhd: Mdhd,
    pub hdlr: Hdlr,
    pub minf: Minf,
}

impl Atom for Mdia {
    const KIND: FourCC = FourCC::new(b"mdia");

    nested! {
        required: [ Mdhd, Hdlr, Minf ],
        optional: [] ,
        multiple: [],
    }
}
