mod mehd;
mod trex;

pub use mehd::*;
pub use trex::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mvex {
    pub mehd: Option<Mehd>,
    pub trex: Vec<Trex>,
}

impl Atom for Mvex {
    const KIND: FourCC = FourCC::new(b"mvex");

    nested! {
        required: [],
        optional: [ Mehd ],
        multiple: [ Trex ],
    }
}
