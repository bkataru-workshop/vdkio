use bytes::Bytes;

#[repr(u8)]
#[derive(Debug, PartialEq)]
/// H.265 NAL unit types
pub enum NALUnitType {
    /// Trailing picture
    Trail = 0,
    /// Instantaneous Decoding Refresh picture
    Idr = 19,    // IDR W RADL
    /// IDR picture Non-leading picture
    IdrNlp = 20, // IDR N LP
    /// Clean Random Access picture
    CraNut = 21, // Clean Random Access
    /// Video Parameter Set
    Vps = 32,
    /// Sequence Parameter Set
    Sps = 33,
    /// Picture Parameter Set
    Pps = 34,
    /// Access Unit Delimiter
    Aud = 35,
    /// End of Sequence NAL unit
    EosNut = 36,
    /// End of Bitstream NAL unit
    EobNut = 37,
    /// Filler Data NAL unit
    FdNut = 38,
    /// Supplemental Enhancement Information, prefix SEI
    PrefixSei = 39,
    /// Supplemental Enhancement Information, suffix SEI
    SuffixSei = 40,
    /// Unspecified NAL unit type
    Unspecified(u8),
}

impl NALUnitType {
    /// Creates a NALUnitType from a u8 value
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

/// H.265 NAL unit structure
#[derive(Debug)]
pub struct NALUnit {
    /// NAL unit type
    pub nal_type: NALUnitType,
    /// NAL unit data (payload)
    pub data: Bytes,
}

impl NALUnit {
    /// Creates a new NALUnit from raw data
    pub fn new(data: Bytes) -> Self {
        let nal_type_byte = (data[0] >> 1) & 0x3F;
        let nal_type = NALUnitType::from_u8(nal_type_byte);
        NALUnit { nal_type, data }
    }
}

/// H.265 Profile Tier Level (PTL) structure
#[derive(Debug, Default)]
pub struct ProfileTierLevel {
    /// Profile space
    pub profile_space: u8,
    /// Tier flag
    pub tier_flag: bool,
    /// Profile IDC
    pub profile_idc: u8,
    /// Profile compatibility flags
    pub profile_compatibility_flags: u32,
    /// Progressive source flag
    pub progressive_source_flag: bool,
    /// Interlaced source flag
    pub interlaced_source_flag: bool,
    /// Non-packed constraint flag
    pub non_packed_constraint_flag: bool,
    /// Frame-only constraint flag
    pub frame_only_constraint_flag: bool,
    /// Level IDC
    pub level_idc: u8,
}

/// H.265 Sequence Parameter Set (SPS) information
#[derive(Debug)]
pub struct SPSInfo {
    /// Sequence Parameter Set ID
    pub sps_id: u32,
    /// Video Parameter Set ID
    pub vps_id: u8,
    /// Chroma format IDC
    pub chroma_format_idc: u32,
    /// Profile Tier Level structure
    pub profile_tier_level: ProfileTierLevel,
    /// Picture width in luma samples
    pub pic_width_in_luma_samples: u32,
    /// Picture height in luma samples
    pub pic_height_in_luma_samples: u32,
    /// Conformance window flag
    pub conformance_window_flag: bool,
    /// Conformance window left offset
    pub conf_win_left_offset: u32,
    /// Conformance window right offset
    pub conf_win_right_offset: u32,
    /// Conformance window top offset
    pub conf_win_top_offset: u32,
    /// Conformance window bottom offset
    pub conf_win_bottom_offset: u32,
    /// Maximum number of sub-layers minus 1
    pub max_sub_layers_minus1: u8,
    /// Temporal ID nesting flag
    pub temporal_id_nesting_flag: bool,
}

/// H.265 Picture Parameter Set (PPS) information
#[derive(Debug)]
pub struct PPSInfo {
    /// Picture Parameter Set ID
    pub pps_id: u32,
    /// Sequence Parameter Set ID
    pub sps_id: u32,
    /// Dependent slice segments enabled flag
    pub dependent_slice_segments_enabled_flag: bool,
    /// Output flag present flag
    pub output_flag_present_flag: bool,
    /// Number of extra slice header bits
    pub num_extra_slice_header_bits: u8,
    /// Sign data hiding enabled flag
    pub sign_data_hiding_enabled_flag: bool,
    /// Context Adaptive Binary Arithmetic Coding (CABAC) initialization present flag
    pub cabac_init_present_flag: bool,
    /// Number of reference index L0 default active minus 1
    pub num_ref_idx_l0_default_active_minus1: u32,
    /// Number of reference index L1 default active minus 1
    pub num_ref_idx_l1_default_active_minus1: u32,
    /// Initial QP minus 26
    pub init_qp_minus26: i32,
    /// Constrained intra prediction flag
    pub constrained_intra_pred_flag: bool,
    /// Transform skip enabled flag
    pub transform_skip_enabled_flag: bool,
}

/// H.265 Video Parameter Set (VPS) information
#[derive(Debug)]
pub struct VPSInfo {
    /// Video Parameter Set ID
    pub vps_id: u8,
    /// Base layer internal flag
    pub base_layer_internal_flag: bool,
    /// Base layer available flag
    pub base_layer_available_flag: bool,
    /// Maximum number of layers minus 1
    pub max_layers_minus1: u8,
    /// Maximum number of sub-layers minus 1
    pub max_sub_layers_minus1: u8,
    /// Temporal ID nesting flag
    pub temporal_id_nesting_flag: bool,
    /// Profile Tier Level structure
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
