use crate::*;

// Based on: https://github.com/gliese1337/HLS.js/blob/master/spsparser/src/index.ts
pub struct Sps {
    pub sps_id: u64,
    pub profile_idc: u8,
    pub level_idc: u8,
    pub profile_compatibility: u8,
    pub frame_mbs_only_flag: bool,
    pub pic_width_in_mbs: u64,
    pub pic_height_in_map_units: u64,
    pub frame_cropping_flag: bool,
    pub frame_cropping: Option<FrameCropping>,

    pub chroma_format_idc: u64,
    pub bit_depth_luma: u64,
    pub bit_depth_chroma: u64,
    pub color_plane_flag: bool,
    pub qpprime_y_zero_transform_bypass_flag: bool,
    pub seq_scaling_matrix_present_flag: bool,
    pub seq_scaling_matrix: Vec<[u8; 16]>,
    pub log2_max_frame_num: u64,
    pub pic_order_cnt_type: u64,
    pub delta_pic_order_always_zero_flag: bool,
    pub offset_for_non_ref_pic: i64,
    pub offset_for_top_to_bottom_field: i64,
    pub offset_for_ref_frame: Vec<i64>,
    pub log2_max_pic_order_cnt_lsb: u64,

    pub max_num_ref_frames: u64,
    pub gaps_in_frame_num_value_allowed_flag: bool,
    pub mb_adaptive_frame_field_flag: bool,
    pub direct_8x8_inference_flag: bool,
    pub vui_parameters_present_flag: bool,
    pub vui_parameters: Option<VUIParams>,
}

impl Decode for Sps {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let profile_idc = u8::decode(buf)?;
        let profile_compatibility = u8::decode(buf)?;
        let level_idc = u8::decode(buf)?;

        let remain = buf.slice(buf.remaining());
        let mut exp = ExpGolombDecoder::new(remain, 0)?;
        let sps_id = exp.next()?;

        let mut chroma_format_idc = 1;
        let mut bit_depth_luma = 0;
        let mut bit_depth_chroma = 0;
        let mut color_plane_flag = false;
        let mut qpprime_y_zero_transform_bypass_flag = false;
        let mut seq_scaling_matrix_present_flag = false;

        let mut scaling_list_4x4 = Vec::new();
        let mut scaling_list_8x8 = Vec::new();

        let mut use_default_scaling_matrix_4x4_flag = [false; 6];
        let mut use_default_scaling_matrix_8x8_flag = [false; 12];

        match profile_idc {
            100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135 => {
                chroma_format_idc = exp.next()?;
                let limit = match chroma_format_idc {
                    3 => {
                        color_plane_flag = exp.next_bit()?;
                        12
                    }
                    _ => 8,
                };
                bit_depth_luma = exp.next()? + 8;
                bit_depth_chroma = exp.next()? + 8;
                qpprime_y_zero_transform_bypass_flag = exp.next_bit()?;
                seq_scaling_matrix_present_flag = exp.next_bit()?;

                // TODO: This scaling_list stuff is undoubtedly wrong.
                // The original code has some major inconsistencies and issues.
                if (seq_scaling_matrix_present_flag) {
                    for i in 0..6 {
                        //seq_scaling_list_present_flag
                        if exp.next_bit()? {
                            let (list, defaults) = scaling_list::<16>(&mut exp)?;
                            scaling_list_4x4.push(list);
                            use_default_scaling_matrix_4x4_flag[i] = defaults;
                        }
                    }

                    for i in 0..limit {
                        //seq_scaling_list_present_flag
                        if exp.next_bit()? {
                            let (list, defaults) = scaling_list::<64>(&mut exp)?;
                            scaling_list_8x8.push(list);
                            use_default_scaling_matrix_8x8_flag[i] = defaults;
                        }
                    }
                }
            }
            _ => {}
        };

        let log2_max_frame_num = exp.next()? + 4;
        let pic_order_cnt_type = exp.next()?;

        let mut delta_pic_order_always_zero_flag = false;
        let mut offset_for_non_ref_pic = 0;
        let mut offset_for_top_to_bottom_field = 0;

        let mut offset_for_ref_frame = Vec::new();

        let mut log2_max_pic_order_cnt_lsb = 0;
        if (pic_order_cnt_type == 0) {
            log2_max_pic_order_cnt_lsb = exp.next()? + 4;
        } else if (pic_order_cnt_type == 1) {
            delta_pic_order_always_zero_flag = exp.next_bit()?;
            offset_for_non_ref_pic = exp.next_i64()?;
            offset_for_top_to_bottom_field = exp.next_i64()?;
            let num_ref_frames_in_pic_order_cnt_cycle = exp.next()?;
            for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
                offset_for_ref_frame.push(exp.next_i64()?);
            }
        }

        let max_num_ref_frames = exp.next()?;
        let gaps_in_frame_num_value_allowed_flag = exp.next_bit()?;
        let pic_width_in_mbs = exp.next()? + 1;
        let pic_height_in_map_units = exp.next()? + 1;
        let frame_mbs_only_flag = exp.next_bit()?;
        let mut mb_adaptive_frame_field_flag = false;
        if (!frame_mbs_only_flag) {
            mb_adaptive_frame_field_flag = exp.next_bit()?;
        }

        let direct_8x8_inference_flag = exp.next_bit()?;
        let frame_cropping_flag = exp.next_bit()?;
        let frame_cropping = frame_cropping_flag
            .then(|| FrameCropping::decode(&mut exp))
            .transpose()?;

        let vui_parameters_present_flag = exp.next_bit()?;
        let vui_parameters = vui_parameters_present_flag
            .then(|| VUIParams::decode(&mut exp))
            .transpose()?;

        Ok(Self {
            sps_id,
            profile_compatibility,
            profile_idc,
            level_idc,
            chroma_format_idc,
            bit_depth_luma,
            bit_depth_chroma,
            color_plane_flag,
            qpprime_y_zero_transform_bypass_flag,
            seq_scaling_matrix_present_flag,
            seq_scaling_matrix: scaling_list_4x4,
            log2_max_frame_num,
            pic_order_cnt_type,
            delta_pic_order_always_zero_flag,
            offset_for_non_ref_pic,
            offset_for_top_to_bottom_field,
            offset_for_ref_frame,
            log2_max_pic_order_cnt_lsb,
            max_num_ref_frames,
            gaps_in_frame_num_value_allowed_flag,
            pic_width_in_mbs,
            pic_height_in_map_units,
            frame_mbs_only_flag,
            mb_adaptive_frame_field_flag,
            direct_8x8_inference_flag,
            frame_cropping_flag,
            frame_cropping,
            vui_parameters_present_flag,
            vui_parameters,
        })
    }
}

fn scaling_list<const N: usize>(exp: &mut ExpGolombDecoder) -> Result<([u8; N], bool)> {
    let mut last_scale: u8 = 8;
    let mut next_scale = 8;
    let mut scaling_list = [0; N];
    let mut use_default = false;

    for j in 0..N {
        if (next_scale != 0) {
            let delta_scale = exp.next_u8()?;
            next_scale = last_scale.overflowing_add(delta_scale).0;
            use_default = (j == 0 && next_scale == 0);
        }
        if (next_scale != 0) {
            last_scale = next_scale;
        }
        scaling_list[j] = last_scale;
    }

    return Ok((scaling_list, use_default));
}

pub struct FrameCropping {
    pub left: u64,
    pub right: u64,
    pub top: u64,
    pub bottom: u64,
}

impl FrameCropping {
    pub fn decode(exp: &mut ExpGolombDecoder) -> Result<Self> {
        let left = exp.next()?;
        let right = exp.next()?;
        let top = exp.next()?;
        let bottom = exp.next()?;

        Ok(Self {
            left,
            right,
            top,
            bottom,
        })
    }
}

pub struct VUIParams {
    pub aspect_ratio_info_present_flag: bool,
    pub aspect_ratio_idc: u64,
    pub sar_width: u64,
    pub sar_height: u64,
    pub overscan_info_present_flag: bool,
    pub overscan_appropriate_flag: bool,
    pub video_signal_type_present_flag: bool,
    pub video_format: u64,
    pub video_full_range_flag: bool,
    pub colour_description_present_flag: bool,
    pub colour_primaries: u64,
    pub transfer_characteristics: u64,
    pub matrix_coefficients: u64,
    pub chroma_loc_info_present_flag: bool,
    pub chroma_sample_loc_type_top_field: u64,
    pub chroma_sample_loc_type_bottom_field: u64,
    pub timing_info_present_flag: bool,
    pub num_units_in_tick: u64,
    pub time_scale: u64,
    pub fixed_frame_rate_flag: bool,
    pub nal_hrd_parameters_present_flag: bool,
    pub vcl_hrd_parameters_present_flag: bool,
    pub hrd_params: HRDParams,
    pub low_delay_hrd_flag: bool,
    pub pic_struct_present_flag: bool,
    pub bitstream_restriction_flag: bool,
    pub motion_vectors_over_pic_boundaries_flag: u64,
    pub max_bytes_per_pic_denom: u64,
    pub max_bits_per_mb_denom: u64,
    pub log2_max_mv_length_horizontal: u64,
    pub log2_max_mv_length_vertical: u64,
    pub num_reorder_frames: u64,
    pub max_dec_frame_buffering: u64,
}

impl VUIParams {
    pub fn decode(exp: &mut ExpGolombDecoder) -> Result<Self> {
        let aspect_ratio_info_present_flag = exp.next_bit()?;
        if aspect_ratio_info_present_flag {
            let aspect_ratio_idc = exp.next()?;
            if (aspect_ratio_idc == 255) {
                let sar_width = exp.next()?;
                let sar_height = exp.next()?;
            }
        }

        let overscan_info_present_flag = exp.next_bit()?;
        if (overscan_info_present_flag) {
            let overscan_appropriate_flag = exp.next_bit()?;
        }

        let video_signal_type_present_flag = exp.next_bit()?;
        if (video_signal_type_present_flag) {
            let video_format = exp.next()?;
            let video_full_range_flag = exp.next_bit()?;
            let colour_description_present_flag = exp.next_bit()?;
            if (colour_description_present_flag) {
                let colour_primaries = exp.next()?;
                let transfer_characteristics = exp.next()?;
                let matrix_coefficients = exp.next()?;
            }
        }

        let chroma_loc_info_present_flag = exp.next_bit()?;
        if (chroma_loc_info_present_flag) {
            let chroma_sample_loc_type_top_field = exp.next()?;
            let chroma_sample_loc_type_bottom_field = exp.next()?;
        }

        let timing_info_present_flag = exp.next_bit()?;
        if (timing_info_present_flag) {
            let num_units_in_tick = exp.next()?;
            let time_scale = exp.next()?;
            let fixed_frame_rate_flag = exp.next_bit()?;
        }

        let nal_hrd_parameters_present_flag = exp.next_bit()?;
        if (nal_hrd_parameters_present_flag) {
            let hrd = HRDParams::decode(exp)?;
        }

        let vcl_hrd_parameters_present_flag = exp.next_bit()?;
        if (vcl_hrd_parameters_present_flag) {
            let hrd = HRDParams::decode(exp)?;
        }

        if (nal_hrd_parameters_present_flag || vcl_hrd_parameters_present_flag) {
            let low_delay_hrd_flag = exp.next_bit()?;
        }

        let pic_struct_present_flag = exp.next_bit()?;
        let bitstream_restriction_flag = exp.next_bit()?;
        if (bitstream_restriction_flag) {
            let motion_vectors_over_pic_boundaries_flag = exp.next_bit()?;
            let max_bytes_per_pic_denom = exp.next()?;
            let max_bits_per_mb_denom = exp.next()?;
            let log2_max_mv_length_horizontal = exp.next()?;
            let log2_max_mv_length_vertical = exp.next()?;
            let num_reorder_frames = exp.next()?;
            let max_dec_frame_buffering = exp.next()?;
        }

        Ok(Self {
            aspect_ratio_info_present_flag,
            aspect_ratio_idc,
            sar_width,
            sar_height,
            overscan_info_present_flag,
            overscan_appropriate_flag,
            video_signal_type_present_flag,
            video_format,
            video_full_range_flag,
            colour_description_present_flag,
            colour_primaries,
            transfer_characteristics,
            matrix_coefficients,
            chroma_loc_info_present_flag,
            chroma_sample_loc_type_top_field,
            chroma_sample_loc_type_bottom_field,
            timing_info_present_flag,
            num_units_in_tick,
            time_scale,
            fixed_frame_rate_flag,
            nal_hrd_parameters_present_flag,
            vcl_hrd_parameters_present_flag,
            hrd_params: hrd,
            low_delay_hrd_flag,
            pic_struct_present_flag,
            bitstream_restriction_flag,
            motion_vectors_over_pic_boundaries_flag,
            max_bytes_per_pic_denom,
            max_bits_per_mb_denom,
            log2_max_mv_length_horizontal,
            log2_max_mv_length_vertical,
            num_reorder_frames,
            max_dec_frame_buffering,
        })
    }
}

pub struct HRDParams {
    pub cpb_cnt: u64,
    pub bit_rate_scale: u64,
    pub cpb_size_scale: u64,
    pub bit_rate_value: Vec<u64>,
    pub cpb_size_value: Vec<u64>,
    pub cbr_flag: Vec<bool>,
    pub initial_cpb_removal_delay_length: u64,
    pub cpb_removal_delay_length: u64,
    pub dpb_output_delay_length: u64,
    pub time_offset_length: u64,
}

impl HRDParams {
    pub fn decode(exp: &mut ExpGolombDecoder) -> Result<Self> {
        let cpb_cnt = exp.next()?;
        let bit_rate_scale = exp.next()?;
        let cpb_size_scale = exp.next()?;

        let mut bit_rate_value = Vec::new();
        let mut cpb_size_value = Vec::new();
        let mut cbr_flag = Vec::new();

        for _ in 0..cpb_cnt {
            bit_rate_value.push(exp.next()?);
            cpb_size_value.push(exp.next()?);
            cbr_flag.push(exp.next_bit()?);
        }

        let initial_cpb_removal_delay_length = exp.next()?;
        let cpb_removal_delay_length = exp.next()?;
        let dpb_output_delay_length = exp.next()?;
        let time_offset_length = exp.next()?;

        Ok(Self {
            cpb_cnt,
            bit_rate_scale,
            cpb_size_scale,
            bit_rate_value,
            cpb_size_value,
            cbr_flag,
            initial_cpb_removal_delay_length,
            cpb_removal_delay_length,
            dpb_output_delay_length,
            time_offset_length,
        })
    }
}
