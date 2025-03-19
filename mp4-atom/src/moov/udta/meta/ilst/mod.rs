mod covr;
mod desc;
mod name;
mod year;

pub use covr::*;
pub use desc::*;
pub use name::*;
pub use year::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ilst {
    pub name: Option<Name>,
    pub year: Option<Year>, // Called day in the spec
    pub covr: Option<Covr>,
    pub desc: Option<Desc>,
}

impl Atom for Ilst {
    const KIND: FourCC = FourCC::new(b"ilst");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        let mut name = None;
        let mut year = None;
        let mut covr = None;
        let mut desc = None;

        while let Some(atom) = Any::decode_maybe(buf)? {
            match atom {
                Any::Name(atom) => name = atom.into(),
                Any::Year(atom) => year = atom.into(),
                Any::Covr(atom) => covr = atom.into(),
                Any::Desc(atom) => desc = atom.into(),
                Any::Unknown(kind, _) => tracing::warn!("unknown atom: {:?}", kind),
                atom => return Err(Error::UnexpectedBox(atom.kind())),
            }
        }

        Ok(Ilst {
            name,
            year,
            covr,
            desc,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.name.encode(buf)?;
        self.year.encode(buf)?;
        self.covr.encode(buf)?;
        self.desc.encode(buf)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ilst() {
        let expected = Ilst {
            year: Year("src_year".to_string()).into(),
            ..Default::default()
        };

        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Ilst::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_ilst_empty() {
        let expected = Ilst::default();
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Ilst::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
