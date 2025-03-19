mod co64;
mod ctts;
mod stco;
mod stsc;
mod stsd;
mod stss;
mod stsz;
mod stts;

pub use co64::*;
pub use ctts::*;
pub use stco::*;
pub use stsc::*;
pub use stsd::*;
pub use stss::*;
pub use stsz::*;
pub use stts::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stbl {
    pub stsd: Stsd,
    pub stts: Stts,
    pub ctts: Option<Ctts>,
    pub stss: Option<Stss>,
    pub stsc: Stsc,
    pub stsz: Stsz,
    pub stco: Option<Stco>,
    pub co64: Option<Co64>,
}

impl Atom for Stbl {
    const KIND: FourCC = FourCC::new(b"stbl");

    nested! {
        required: [ Stsd, Stts, Stsc, Stsz ],
        optional: [ Ctts, Stss, Stco, Co64 ],
        multiple: [],
    }
}
