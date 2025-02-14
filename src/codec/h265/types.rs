use bytes::Bytes;

#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum NALUnitType {
    Trail = 0,
    Idr = 19,    // IDR W RADL
    IdrNlp = 20, // IDR N LP
    CraNut = 21, // Clean Random Access
    Vps = 32,
    Sps = 33,
    Pps = 34,
    Aud = 35,
    EosNut = 36,
    EobNut = 37,
    FdNut = 38,
    PrefixSei = 39,
    SuffixSei = 40,
    Unspecified(u8),
}

impl NALUnitType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => NALUnitType::Trail,
            19 => NALUnitType::Idr,
            20 => NALUnitType::IdrNlp,
            21 => NALUnitType::CraNut,
            32 => NALUnitType::Vps,
            33 => NALUnitType::Sps,
            34 => NALUnitType::Pps,
            35 => NALUnitType::Aud,
            36 => NALUnitType::EosNut,
            37 => NALUnitType::EobNut,
            38 => NALUnitType::FdNut,
            39 => NALUnitType::PrefixSei,
            40 => NALUnitType::SuffixSei,
            _ => NALUnitType::Unspecified(value),
        }
    }
}

#[derive(Debug)]
pub struct NALUnit {
    pub nal_type: NALUnitType,
    pub data: Bytes,
}

impl NALUnit {
    pub fn new(data: Bytes) -> Self {
        let nal_type_byte = (data[0] >> 1) & 0x3F;
        let nal_type = NALUnitType::from_u8(nal_type_byte);
        NALUnit {
            nal_type,
            data,
        }
    }
}

#[derive(Debug, Default)]
pub struct ProfileTierLevel {
    pub profile_space: u8,
    pub tier_flag: bool,
    pub profile_idc: u8,
    pub profile_compatibility_flags: u32,
    pub progressive_source_flag: bool,
    pub interlaced_source_flag: bool,
    pub non_packed_constraint_flag: bool,
    pub frame_only_constraint_flag: bool,
    pub level_idc: u8,
}

#[derive(Debug)]
pub struct SPSInfo {
    pub sps_id: u32,
    pub vps_id: u8,
    pub chroma_format_idc: u32,
    pub profile_tier_level: ProfileTierLevel,
    pub pic_width_in_luma_samples: u32,
    pub pic_height_in_luma_samples: u32,
    pub conformance_window_flag: bool,
    pub conf_win_left_offset: u32,
    pub conf_win_right_offset: u32,
    pub conf_win_top_offset: u32,
    pub conf_win_bottom_offset: u32,
    pub max_sub_layers_minus1: u8,
    pub temporal_id_nesting_flag: bool,
}

#[derive(Debug)]
pub struct PPSInfo {
    pub pps_id: u32,
    pub sps_id: u32,
    pub dependent_slice_segments_enabled_flag: bool,
    pub output_flag_present_flag: bool,
    pub num_extra_slice_header_bits: u8,
    pub sign_data_hiding_enabled_flag: bool,
    pub cabac_init_present_flag: bool,
    pub num_ref_idx_l0_default_active_minus1: u32,
    pub num_ref_idx_l1_default_active_minus1: u32,
    pub init_qp_minus26: i32,
    pub constrained_intra_pred_flag: bool,
    pub transform_skip_enabled_flag: bool,
}

#[derive(Debug)]
pub struct VPSInfo {
    pub vps_id: u8,
    pub base_layer_internal_flag: bool,
    pub base_layer_available_flag: bool,
    pub max_layers_minus1: u8,
    pub max_sub_layers_minus1: u8,
    pub temporal_id_nesting_flag: bool,
    pub profile_tier_level: ProfileTierLevel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nal_unit_type_from_u8() {
        assert_eq!(NALUnitType::from_u8(0), NALUnitType::Trail);
        assert_eq!(NALUnitType::from_u8(19), NALUnitType::Idr);
        assert_eq!(NALUnitType::from_u8(33), NALUnitType::Sps);
        assert_eq!(NALUnitType::from_u8(255), NALUnitType::Unspecified(255));
    }

    #[test]
    fn test_nal_unit_creation() {
        let data = Bytes::from(vec![0x40, 0x01, 0x02, 0x03]); // First byte: 0x40 = NAL type 32 (VPS)
        let nalu = NALUnit::new(data);
        assert_eq!(nalu.nal_type, NALUnitType::Vps);
    }
}
