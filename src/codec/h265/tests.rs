use super::*;
use bytes::Bytes;

fn create_test_nalu(nalu_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut data = vec![
        (nalu_type << 1) & 0x7E, // NAL unit header (1 byte)
        0x01, // Layer ID and temporal ID
    ];
    data.extend_from_slice(payload);
    data
}

#[test]
fn test_vps_parsing() {
    let mut parser = H265Parser::new();
    
    // Create a minimal valid VPS
    let vps_payload = vec![
        0x40, // vps_id=4, base_layer_internal_flag=0, base_layer_available_flag=0
        0x01, // max_layers=1, max_sub_layers=1
        0x00, 0x00, // reserved
        0x40, 0x00, 0x00, 0x00, // profile_space=1, tier_flag=0, profile_idc=0
        0x00, 0x00, 0x00, 0x00, // compatibility flags
        0x00, // progressive/interlaced flags
        0x60, // reserved + level_idc=96
    ];

    let vps_nalu = create_test_nalu(32, &vps_payload);
    let result = parser.parse_nalu(&vps_nalu).unwrap();
    
    assert_eq!(result.nal_type, NALUnitType::Vps);
    
    if let Some(vps) = parser.vps {
        assert_eq!(vps.vps_id, 4);
        assert_eq!(vps.max_layers_minus1, 0);
        assert_eq!(vps.max_sub_layers_minus1, 0);
        assert_eq!(vps.profile_tier_level.profile_space, 1);
        assert_eq!(vps.profile_tier_level.level_idc, 96);
    } else {
        panic!("VPS parsing failed");
    }
}

#[test]
fn test_sps_parsing() {
    let mut parser = H265Parser::new();
    
    // Create a minimal valid SPS
    let sps_payload = vec![
        0x01, // vps_id=0, max_sub_layers=1
        0x40, 0x00, 0x00, 0x00, // profile_space=1, tier_flag=0, profile_idc=0
        0x00, 0x00, 0x00, 0x00, // compatibility flags
        0x00, // progressive/interlaced flags
        0x60, // reserved + level_idc=96
        0x00, // sps_id=0 (exp-golomb)
        0x01, // chroma_format_idc=1 (exp-golomb)
        0xa0, 0x02, // pic_width=320 (exp-golomb)
        0x80, 0x02, // pic_height=240 (exp-golomb)
        0x00, // conformance_window_flag=0
    ];

    let sps_nalu = create_test_nalu(33, &sps_payload);
    let result = parser.parse_nalu(&sps_nalu).unwrap();
    
    assert_eq!(result.nal_type, NALUnitType::Sps);
    
    if let Some(sps) = parser.sps {
        assert_eq!(sps.vps_id, 0);
        assert_eq!(sps.sps_id, 0);
        assert_eq!(sps.chroma_format_idc, 1);
        assert_eq!(sps.pic_width_in_luma_samples, 320);
        assert_eq!(sps.pic_height_in_luma_samples, 240);
        assert_eq!(sps.profile_tier_level.level_idc, 96);
    } else {
        panic!("SPS parsing failed");
    }
}

#[test]
fn test_pps_parsing() {
    let mut parser = H265Parser::new();
    
    // Create a minimal valid PPS
    let pps_payload = vec![
        0x00, // pps_id=0 (exp-golomb)
        0x00, // sps_id=0 (exp-golomb)
        0x08, // dependent_slice_enabled=0, output_flag_present=0, num_extra_slice_header_bits=1
        0x00, // sign_data_hiding=0, cabac_init_present=0
        0x00, // num_ref_idx_l0=0 (exp-golomb)
        0x00, // num_ref_idx_l1=0 (exp-golomb)
        0x01, // init_qp_minus26=0 (exp-golomb)
        0x00, // constrained_intra_pred=0, transform_skip_enabled=0
    ];

    let pps_nalu = create_test_nalu(34, &pps_payload);
    let result = parser.parse_nalu(&pps_nalu).unwrap();
    
    assert_eq!(result.nal_type, NALUnitType::Pps);
    
    if let Some(pps) = parser.pps {
        assert_eq!(pps.pps_id, 0);
        assert_eq!(pps.sps_id, 0);
        assert_eq!(pps.num_extra_slice_header_bits, 1);
        assert_eq!(pps.init_qp_minus26, 0);
        assert!(!pps.dependent_slice_segments_enabled_flag);
        assert!(!pps.transform_skip_enabled_flag);
    } else {
        panic!("PPS parsing failed");
    }
}

#[test]
fn test_nal_unit_header_parsing() {
    let mut parser = H265Parser::new();
    
    // Test different NAL unit types
    let test_cases = vec![
        (0x40, NALUnitType::Vps),       // VPS
        (0x42, NALUnitType::Sps),       // SPS
        (0x44, NALUnitType::Pps),       // PPS
        (0x02, NALUnitType::Trail),     // Trail N
        (0x26, NALUnitType::Idr), // IDR_W_RADL
        (0x28, NALUnitType::IdrNlp), // IDR_N_LP
        (0x2A, NALUnitType::CraNut),    // CRA_NUT
    ];

    for (header_byte, expected_type) in test_cases {
        let nalu_data = vec![header_byte, 0x01, 0x00]; // Simple NAL unit
        let result = parser.parse_nalu(&nalu_data).unwrap();
        assert_eq!(result.nal_type, expected_type);
    }
}

#[test]
fn test_emulation_prevention() {
    let mut parser = H265Parser::new();
    
    let test_cases = vec![
        // Test case 1: Basic emulation prevention
        (
            vec![0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x03],
            vec![0x00, 0x00, 0x00, 0x00, 0x03],
        ),
        // Test case 2: Multiple emulation prevention bytes
        (
            vec![0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x03, 0x00],
            vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        ),
        // Test case 3: Emulation prevention at the end
        (
            vec![0x01, 0x00, 0x00, 0x03],
            vec![0x01, 0x00, 0x00],
        ),
        // Test case 4: No emulation prevention needed
        (
            vec![0x00, 0x01, 0x02, 0x03],
            vec![0x00, 0x01, 0x02, 0x03],
        ),
    ];

    for (input, expected) in test_cases {
        let result = parser.remove_emulation_prevention(&input);
        assert_eq!(result, expected);
    }
}