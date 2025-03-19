use crate::*;

ext! {
    name: Tfhd,
    versions: [0],
    flags: {
        base_data_offset = 0,
        sample_description_index = 1,
        default_sample_duration = 3,
        default_sample_size = 4,
        default_sample_flags = 5,
        duration_is_empty = 16,
        default_base_is_moof = 17,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tfhd {
    pub track_id: u32,
    pub base_data_offset: Option<u64>,
    pub sample_description_index: Option<u32>,
    pub default_sample_duration: Option<u32>,
    pub default_sample_size: Option<u32>,
    pub default_sample_flags: Option<u32>,
}

impl AtomExt for Tfhd {
    const KIND_EXT: FourCC = FourCC::new(b"tfhd");

    type Ext = TfhdExt;

    fn decode_body_ext<B: Buf>(buf: &mut B, ext: TfhdExt) -> Result<Self> {
        let track_id = u32::decode(buf)?;

        let base_data_offset = match ext.base_data_offset {
            true => u64::decode(buf)?.into(),
            false => None,
        };

        let sample_description_index = match ext.sample_description_index {
            true => u32::decode(buf)?.into(),
            false => None,
        };

        let default_sample_duration = match ext.default_sample_duration {
            true => u32::decode(buf)?.into(),
            false => None,
        };

        let default_sample_size = match ext.default_sample_size {
            true => u32::decode(buf)?.into(),
            false => None,
        };

        let default_sample_flags = match ext.default_sample_flags {
            true => u32::decode(buf)?.into(),
            false => None,
        };

        Ok(Tfhd {
            track_id,
            base_data_offset,
            sample_description_index,
            default_sample_duration,
            default_sample_size,
            default_sample_flags,
        })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<TfhdExt> {
        let ext = TfhdExt {
            base_data_offset: self.base_data_offset.is_some(),
            sample_description_index: self.sample_description_index.is_some(),
            default_sample_duration: self.default_sample_duration.is_some(),
            default_sample_size: self.default_sample_size.is_some(),
            default_sample_flags: self.default_sample_flags.is_some(),
            ..Default::default()
        };

        self.track_id.encode(buf)?;
        self.base_data_offset.encode(buf)?;
        self.sample_description_index.encode(buf)?;
        self.default_sample_duration.encode(buf)?;
        self.default_sample_size.encode(buf)?;
        self.default_sample_flags.encode(buf)?;

        Ok(ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfhd() {
        let expected = Tfhd {
            track_id: 1,
            base_data_offset: None,
            sample_description_index: None,
            default_sample_duration: None,
            default_sample_size: None,
            default_sample_flags: None,
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Tfhd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_tfhd_with_flags() {
        let expected = Tfhd {
            track_id: 1,
            base_data_offset: None,
            sample_description_index: Some(1),
            default_sample_duration: Some(512),
            default_sample_size: None,
            default_sample_flags: Some(0x1010000),
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Tfhd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
