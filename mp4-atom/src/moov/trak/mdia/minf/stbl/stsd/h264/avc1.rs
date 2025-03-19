use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Avc1 {
    pub visual: Visual,
    pub avcc: Avcc,
}

impl Atom for Avc1 {
    const KIND: FourCC = FourCC::new(b"avc1");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        let visual = Visual::decode(buf)?;

        let mut avcc = None;
        while let Some(atom) = Any::decode_maybe(buf)? {
            match atom {
                Any::Avcc(atom) => avcc = atom.into(),
                _ => tracing::warn!("unknown atom: {:?}", atom),
            }
        }

        Ok(Avc1 {
            visual,
            avcc: avcc.ok_or(Error::MissingBox(Avcc::KIND))?,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.visual.encode(buf)?;
        self.avcc.encode(buf)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avc1() {
        let expected = Avc1 {
            visual: Visual {
                data_reference_index: 1,
                width: 320,
                height: 240,
                horizresolution: 0x48.into(),
                vertresolution: 0x48.into(),
                frame_count: 1,
                compressor: "ya boy".into(),
                depth: 24,
            },
            avcc: Avcc {
                configuration_version: 1,
                avc_profile_indication: 100,
                profile_compatibility: 0,
                avc_level_indication: 13,
                length_size: 4,
                sequence_parameter_sets: vec![vec![
                    0x67, 0x64, 0x00, 0x0D, 0xAC, 0xD9, 0x41, 0x41, 0xFA, 0x10, 0x00, 0x00, 0x03,
                    0x00, 0x10, 0x00, 0x00, 0x03, 0x03, 0x20, 0xF1, 0x42, 0x99, 0x60,
                ]],
                picture_parameter_sets: vec![vec![0x68, 0xEB, 0xE3, 0xCB, 0x22, 0xC0]],
                ..Default::default()
            },
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Avc1::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
