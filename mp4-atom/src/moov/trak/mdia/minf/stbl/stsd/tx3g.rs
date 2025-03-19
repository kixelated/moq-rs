use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tx3g {
    pub data_reference_index: u16,
    pub display_flags: u32,
    pub horizontal_justification: i8,
    pub vertical_justification: i8,
    pub bg_color_rgba: RgbaColor,
    pub box_record: [i16; 4],
    pub style_record: [u8; 12],
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Default for Tx3g {
    fn default() -> Self {
        Tx3g {
            data_reference_index: 0,
            display_flags: 0,
            horizontal_justification: 1,
            vertical_justification: -1,
            bg_color_rgba: RgbaColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255,
            },
            box_record: [0, 0, 0, 0],
            style_record: [0, 0, 0, 0, 0, 1, 0, 16, 255, 255, 255, 255],
        }
    }
}

impl Atom for Tx3g {
    const KIND: FourCC = FourCC::new(b"tx3g");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        u32::decode(buf)?; // reserved
        u16::decode(buf)?; // reserved
        let data_reference_index = u16::decode(buf)?;

        let display_flags = u32::decode(buf)?;
        let horizontal_justification = i8::decode(buf)?;
        let vertical_justification = i8::decode(buf)?;
        let bg_color_rgba = RgbaColor {
            red: u8::decode(buf)?,
            green: u8::decode(buf)?,
            blue: u8::decode(buf)?,
            alpha: u8::decode(buf)?,
        };
        let box_record: [i16; 4] = [
            i16::decode(buf)?,
            i16::decode(buf)?,
            i16::decode(buf)?,
            i16::decode(buf)?,
        ];
        let style_record = <[u8; 12]>::decode(buf)?;

        Ok(Tx3g {
            data_reference_index,
            display_flags,
            horizontal_justification,
            vertical_justification,
            bg_color_rgba,
            box_record,
            style_record,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        0u32.encode(buf)?; // reserved
        0u16.encode(buf)?; // reserved
        self.data_reference_index.encode(buf)?;
        self.display_flags.encode(buf)?;
        self.horizontal_justification.encode(buf)?;
        self.vertical_justification.encode(buf)?;
        self.bg_color_rgba.red.encode(buf)?;
        self.bg_color_rgba.green.encode(buf)?;
        self.bg_color_rgba.blue.encode(buf)?;
        self.bg_color_rgba.alpha.encode(buf)?;
        for n in 0..4 {
            (self.box_record[n]).encode(buf)?;
        }
        for n in 0..12 {
            (self.style_record[n]).encode(buf)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx3g() {
        let expected = Tx3g {
            data_reference_index: 1,
            display_flags: 0,
            horizontal_justification: 1,
            vertical_justification: -1,
            bg_color_rgba: RgbaColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255,
            },
            box_record: [0, 0, 0, 0],
            style_record: [0, 0, 0, 0, 0, 1, 0, 16, 255, 255, 255, 255],
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Tx3g::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
