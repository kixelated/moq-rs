use crate::*;

// https://www.webmproject.org/vp9/mp4/
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vp08 {
    pub visual: Visual,
    pub vpcc: VpcC,
}

impl Atom for Vp08 {
    const KIND: FourCC = FourCC::new(b"vp08");

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
