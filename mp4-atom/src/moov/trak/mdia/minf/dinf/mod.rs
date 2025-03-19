mod dref;
pub use dref::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dinf {
    pub dref: Dref,
}

impl Atom for Dinf {
    const KIND: FourCC = FourCC::new(b"dinf");

    nested! {
        required: [ Dref ],
        optional: [],
        multiple: [],
    }
}
