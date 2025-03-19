mod meta;
mod skip;

pub use meta::*;
pub use skip::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Udta {
    pub meta: Option<Meta>,
    pub skip: Option<Skip>,
}

impl Atom for Udta {
    const KIND: FourCC = FourCC::new(b"udta");

    nested! {
        required: [ ],
        optional: [ Meta, Skip ],
        multiple: [ ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_udta_empty() {
        let expected = Udta {
            meta: None,
            skip: None,
        };

        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let output = Udta::decode(&mut buf).unwrap();
        assert_eq!(output, expected);
    }

    #[test]
    fn test_udta() {
        let expected = Udta {
            meta: Some(Meta::default()),
            skip: None,
        };

        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let output = Udta::decode(&mut buf).unwrap();
        assert_eq!(output, expected);
    }
}
