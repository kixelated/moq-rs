mod tfdt;
mod tfhd;
mod trun;

pub use tfdt::*;
pub use tfhd::*;
pub use trun::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Traf {
    pub tfhd: Tfhd,
    pub tfdt: Option<Tfdt>,
    pub trun: Option<Trun>,
}

impl Atom for Traf {
    const KIND: FourCC = FourCC::new(b"traf");

    nested! {
        required: [ Tfhd ],
        optional: [ Tfdt, Trun ],
        multiple: [],
    }
}
