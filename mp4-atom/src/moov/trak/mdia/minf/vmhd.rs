use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vmhd {
    pub graphics_mode: u16,
    pub op_color: RgbColor,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RgbColor {
    pub red: u16,
    pub green: u16,
    pub blue: u16,
}

impl AtomExt for Vmhd {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"vmhd");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let graphics_mode = u16::decode(buf)?;
        let op_color = RgbColor {
            red: u16::decode(buf)?,
            green: u16::decode(buf)?,
            blue: u16::decode(buf)?,
        };

        Ok(Vmhd {
            graphics_mode,
            op_color,
        })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.graphics_mode.encode(buf)?;
        self.op_color.red.encode(buf)?;
        self.op_color.green.encode(buf)?;
        self.op_color.blue.encode(buf)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vmhd() {
        let expected = Vmhd {
            graphics_mode: 0,
            op_color: RgbColor {
                red: 0,
                green: 0,
                blue: 0,
            },
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Vmhd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
