use crate::*;

// https://www.webmproject.org/vp9/mp4/
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vp09 {
    pub visual: Visual,
    pub vpcc: VpcC,
}

impl Atom for Vp09 {
    const KIND: FourCC = FourCC::new(b"vp09");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        let visual = Visual::decode(buf)?;

        let mut vpcc = None;
        while let Some(atom) = Any::decode_maybe(buf)? {
            match atom {
                Any::VpcC(atom) => vpcc = atom.into(),
                _ => tracing::warn!("unknown atom: {:?}", atom),
            }
        }

        Ok(Self {
            visual,
            vpcc: vpcc.ok_or(Error::MissingBox(VpcC::KIND))?,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.visual.encode(buf)?;
        self.vpcc.encode(buf)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vp09() {
        let expected = Vp09 {
            visual: Visual {
                width: 1920,
                height: 1080,
                ..Default::default()
            },
            vpcc: VpcC::default(),
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Vp09::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
