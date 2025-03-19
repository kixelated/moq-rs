use crate::*;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hvcc {
    pub configuration_version: u8,
    pub general_profile_space: u8,
    pub general_tier_flag: bool,
    pub general_profile_idc: u8,
    pub general_profile_compatibility_flags: [u8; 4],
    pub general_constraint_indicator_flags: [u8; 6],
    pub general_level_idc: u8,
    pub min_spatial_segmentation_idc: u16,
    pub parallelism_type: u8,
    pub chroma_format_idc: u8,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub avg_frame_rate: u16,
    pub constant_frame_rate: u8,
    pub num_temporal_layers: u8,
    pub temporal_id_nested: bool,
    pub length_size_minus_one: u8,
    pub arrays: Vec<HvcCArray>,
}

impl Hvcc {
    pub fn new() -> Self {
        Self {
            configuration_version: 1,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HvcCArray {
    pub completeness: bool,
    pub nal_unit_type: u8,
    pub nalus: Vec<Vec<u8>>,
}

impl Atom for Hvcc {
    const KIND: FourCC = FourCC::new(b"hvcC");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        let configuration_version = u8::decode(buf)?;
        let params = u8::decode(buf)?;
        let general_profile_space = params & (0b11000000 >> 6);
        let general_tier_flag = (params & (0b00100000 >> 5)) > 0;
        let general_profile_idc = params & 0b00011111;

        let general_profile_compatibility_flags = <[u8; 4]>::decode(buf)?;
        let general_constraint_indicator_flags = <[u8; 6]>::decode(buf)?;
        let general_level_idc = u8::decode(buf)?;
        let min_spatial_segmentation_idc = u16::decode(buf)? & 0x0FFF;
        let parallelism_type = u8::decode(buf)? & 0b11;
        let chroma_format_idc = u8::decode(buf)? & 0b11;
        let bit_depth_luma_minus8 = u8::decode(buf)? & 0b111;
        let bit_depth_chroma_minus8 = u8::decode(buf)? & 0b111;
        let avg_frame_rate = u16::decode(buf)?;

        let params = u8::decode(buf)?;
        let constant_frame_rate = params & (0b11000000 >> 6);
        let num_temporal_layers = params & (0b00111000 >> 3);
        let temporal_id_nested = (params & (0b00000100 >> 2)) > 0;
        let length_size_minus_one = params & 0b000011;

        let num_of_arrays = u8::decode(buf)?;

        let mut arrays = Vec::with_capacity(num_of_arrays as _);
        for _ in 0..num_of_arrays {
            let params = u8::decode(buf)?;
            let num_nalus = u16::decode(buf)?;
            let mut nalus = Vec::with_capacity(num_nalus as usize);

            for _ in 0..num_nalus {
                let size = u16::decode(buf)? as usize;
                let data = Vec::decode_exact(buf, size)?;
                nalus.push(data)
            }

            arrays.push(HvcCArray {
                completeness: (params & 0b10000000) > 0,
                nal_unit_type: params & 0b111111,
                nalus,
            });
        }

        Ok(Hvcc {
            configuration_version,
            general_profile_space,
            general_tier_flag,
            general_profile_idc,
            general_profile_compatibility_flags,
            general_constraint_indicator_flags,
            general_level_idc,
            min_spatial_segmentation_idc,
            parallelism_type,
            chroma_format_idc,
            bit_depth_luma_minus8,
            bit_depth_chroma_minus8,
            avg_frame_rate,
            constant_frame_rate,
            num_temporal_layers,
            temporal_id_nested,
            length_size_minus_one,
            arrays,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.configuration_version.encode(buf)?;
        let general_profile_space = (self.general_profile_space & 0b11) << 6;
        let general_tier_flag = u8::from(self.general_tier_flag) << 5;
        let general_profile_idc = self.general_profile_idc & 0b11111;

        (general_profile_space | general_tier_flag | general_profile_idc).encode(buf)?;
        self.general_profile_compatibility_flags.encode(buf)?;
        self.general_constraint_indicator_flags.encode(buf)?;
        self.general_level_idc.encode(buf)?;

        (self.min_spatial_segmentation_idc & 0x0FFF).encode(buf)?;
        (self.parallelism_type & 0b11).encode(buf)?;
        (self.chroma_format_idc & 0b11).encode(buf)?;
        (self.bit_depth_luma_minus8 & 0b111).encode(buf)?;
        (self.bit_depth_chroma_minus8 & 0b111).encode(buf)?;
        self.avg_frame_rate.encode(buf)?;

        let constant_frame_rate = (self.constant_frame_rate & 0b11) << 6;
        let num_temporal_layers = (self.num_temporal_layers & 0b111) << 3;
        let temporal_id_nested = u8::from(self.temporal_id_nested) << 2;
        let length_size_minus_one = self.length_size_minus_one & 0b11;
        (constant_frame_rate | num_temporal_layers | temporal_id_nested | length_size_minus_one)
            .encode(buf)?;
        (self.arrays.len() as u8).encode(buf)?;
        for arr in &self.arrays {
            ((arr.nal_unit_type & 0b111111) | (u8::from(arr.completeness) << 7)).encode(buf)?;
            (arr.nalus.len() as u16).encode(buf)?;

            for nalu in &arr.nalus {
                (nalu.len() as u16).encode(buf)?;
                nalu.encode(buf)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hev1() {
        let expected = Hev1 {
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
            hvcc: Hvcc {
                configuration_version: 1,
                ..Default::default()
            },
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Hev1::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
