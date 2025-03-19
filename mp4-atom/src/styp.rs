use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Styp {
    pub major_brand: FourCC,
    pub minor_version: u32,
    pub compatible_brands: Vec<FourCC>,
}

impl Atom for Styp {
    const KIND: FourCC = FourCC::new(b"styp");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Styp {
            major_brand: FourCC::decode(buf)?,
            minor_version: u32::decode(buf)?,
            compatible_brands: Vec::<FourCC>::decode(buf)?,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.major_brand.encode(buf)?;
        self.minor_version.encode(buf)?;
        self.compatible_brands.encode(buf)?;
        Ok(())
    }
}
