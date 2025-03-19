mod dinf;
mod smhd;
mod stbl;
mod vmhd;

pub use dinf::*;
pub use smhd::*;
pub use stbl::*;
pub use vmhd::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Minf {
    pub vmhd: Option<Vmhd>,
    pub smhd: Option<Smhd>,
    pub dinf: Dinf,
    pub stbl: Stbl,
}

impl Atom for Minf {
    const KIND: FourCC = FourCC::new(b"minf");

    nested! {
        required: [ Dinf, Stbl ],
        optional: [ Vmhd, Smhd ],
        multiple: [],
    }
}
