use crate::error::Result;
use bytes::{BufMut, Bytes, BytesMut};
use log;

use super::types::{NALUnit, NALUnitType, PPSInfo, ProfileTierLevel, SPSInfo, VPSInfo};
use crate::utils::bits::BitReader;

#[derive(Debug)]
pub struct H265Parser {
    sps: Option<SPSInfo>,
    pps: Option<PPSInfo>,
    vps: Option<VPSInfo>,
    buffer: BytesMut,
}

impl H265Parser {
    pub fn new() -> Self {
        Self {
            sps: None,
            pps: None,
            vps: None,
            buffer: BytesMut::new(),
        }
    }

    pub fn parse_nalu(&mut self, data: &[u8]) -> Result<NALUnit> {
        let data = self.remove_emulation_prevention(data);
        let data = Bytes::from(data);
        let nalu = NALUnit::new(data.clone());
        log::info!("Parsed NAL unit type: {:?}", nalu.nal_type);

        match &nalu.nal_type {
            NALUnitType::Vps => {
                self.vps = Some(self.parse_vps(&data[2..])?);
            }
            NALUnitType::Sps => {
                self.sps = Some(self.parse_sps(&data[2..])?);
            }
            NALUnitType::Pps => {
                self.pps = Some(self.parse_pps(&data[2..])?);
            }
            _ => {}
        }

        Ok(nalu)
    }

    pub fn remove_emulation_prevention(&mut self, data: &[u8]) -> Vec<u8> {
        self.buffer.clear();
        let mut i = 0;

        while i < data.len() {
            if i + 2 < data.len() && data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x03 {
                self.buffer.put_u8(0x00);
                self.buffer.put_u8(0x00);
                i += 3;
                continue;
            }
            self.buffer.put_u8(data[i]);
            i += 1;
        }

        self.buffer.to_vec()
    }

    pub fn is_keyframe(&self, nalu: &NALUnit) -> bool {
        matches!(
            nalu.nal_type,
            NALUnitType::Idr | NALUnitType::IdrNlp | NALUnitType::CraNut
        )
    }

    fn parse_profile_tier_level(
        &mut self,
        reader: &mut BitReader,
        profile_present_flag: bool,
    ) -> Result<ProfileTierLevel> {
        let mut ptl = ProfileTierLevel::default();

        if profile_present_flag {
            ptl.profile_space = reader.read_bits(2)? as u8;
            ptl.tier_flag = reader.read_bit()?;
            ptl.profile_idc = reader.read_bits(5)? as u8;

            // profile_compatibility_flags (32 bits)
            ptl.profile_compatibility_flags = reader.read_bits(32)?;

            ptl.progressive_source_flag = reader.read_bit()?;
            ptl.interlaced_source_flag = reader.read_bit()?;
            ptl.non_packed_constraint_flag = reader.read_bit()?;
            ptl.frame_only_constraint_flag = reader.read_bit()?;

            // Skip reserved bits
            reader.skip_bits(44)?;
        }

        ptl.level_idc = reader.read_bits(8)? as u8;

        Ok(ptl)
    }

    fn parse_vps(&mut self, data: &[u8]) -> Result<VPSInfo> {
        let mut reader = BitReader::new(data);

        let vps_id = reader.read_bits(4)? as u8;
        let base_layer_internal_flag = reader.read_bit()?;
        let base_layer_available_flag = reader.read_bit()?;
        let max_layers_minus1 = reader.read_bits(6)? as u8;
        let max_sub_layers_minus1 = reader.read_bits(3)? as u8;
        let temporal_id_nesting_flag = reader.read_bit()?;

        // Skip reserved bits
        reader.skip_bits(16)?;

        // Parse profile_tier_level
        let profile_tier_level = self.parse_profile_tier_level(&mut reader, true)?;

        Ok(VPSInfo {
            vps_id,
            base_layer_internal_flag,
            base_layer_available_flag,
            max_layers_minus1,
            max_sub_layers_minus1,
            temporal_id_nesting_flag,
            profile_tier_level,
        })
    }

    fn parse_sps(&mut self, data: &[u8]) -> Result<SPSInfo> {
        let mut reader = BitReader::new(data);

        let vps_id = reader.read_bits(4)? as u8;
        let max_sub_layers_minus1 = reader.read_bits(3)? as u8;
        let temporal_id_nesting_flag = reader.read_bit()?;

        // Parse profile_tier_level
        let profile_tier_level = self.parse_profile_tier_level(&mut reader, true)?;

        let sps_id = reader.read_golomb()? as u32;
        let chroma_format_idc = reader.read_golomb()? as u32;

        if chroma_format_idc == 3 {
            let _separate_colour_plane_flag = reader.read_bit()?;
        }

        let pic_width_in_luma_samples = reader.read_golomb()? as u32;
        let pic_height_in_luma_samples = reader.read_golomb()? as u32;

        let conformance_window_flag = reader.read_bit()?;
        let mut conf_win_left_offset = 0;
        let mut conf_win_right_offset = 0;
        let mut conf_win_top_offset = 0;
        let mut conf_win_bottom_offset = 0;

        if conformance_window_flag {
            conf_win_left_offset = reader.read_golomb()? as u32;
            conf_win_right_offset = reader.read_golomb()? as u32;
            conf_win_top_offset = reader.read_golomb()? as u32;
            conf_win_bottom_offset = reader.read_golomb()? as u32;
        }

        Ok(SPSInfo {
            sps_id,
            vps_id,
            chroma_format_idc,
            profile_tier_level,
            pic_width_in_luma_samples,
            pic_height_in_luma_samples,
            conformance_window_flag,
            conf_win_left_offset,
            conf_win_right_offset,
            conf_win_top_offset,
            conf_win_bottom_offset,
            max_sub_layers_minus1,
            temporal_id_nesting_flag,
        })
    }

    fn parse_pps(&mut self, data: &[u8]) -> Result<PPSInfo> {
        let mut reader = BitReader::new(data);

        let pps_id = reader.read_golomb()? as u32;
        let sps_id = reader.read_golomb()? as u32;

        let dependent_slice_segments_enabled_flag = reader.read_bit()?;
        let output_flag_present_flag = reader.read_bit()?;
        let num_extra_slice_header_bits = reader.read_bits(3)? as u8;
        let sign_data_hiding_enabled_flag = reader.read_bit()?;
        let cabac_init_present_flag = reader.read_bit()?;

        let num_ref_idx_l0_default_active_minus1 = reader.read_golomb()? as u32;
        let num_ref_idx_l1_default_active_minus1 = reader.read_golomb()? as u32;

        let init_qp_minus26 = reader.read_signed_golomb()?;
        let constrained_intra_pred_flag = reader.read_bit()?;
        let transform_skip_enabled_flag = reader.read_bit()?;

        Ok(PPSInfo {
            pps_id,
            sps_id,
            dependent_slice_segments_enabled_flag,
            output_flag_present_flag,
            num_extra_slice_header_bits,
            sign_data_hiding_enabled_flag,
            cabac_init_present_flag,
            num_ref_idx_l0_default_active_minus1,
            num_ref_idx_l1_default_active_minus1,
            init_qp_minus26,
            constrained_intra_pred_flag,
            transform_skip_enabled_flag,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_emulation_prevention() {
        let mut parser = H265Parser::new();
        let input = vec![0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x03];
        let output = parser.remove_emulation_prevention(&input);
        assert_eq!(output, vec![0x00, 0x00, 0x00, 0x00, 0x03]);
    }

    #[test]
    fn test_is_keyframe() {
        let parser = H265Parser::new();
        let idr_nalu = NALUnit {
            nal_type: NALUnitType::Idr,
            data: Bytes::new(),
        };
        let non_idr_nalu = NALUnit {
            nal_type: NALUnitType::Trail,
            data: Bytes::new(),
        };

        assert!(parser.is_keyframe(&idr_nalu));
        assert!(!parser.is_keyframe(&non_idr_nalu));
    }
}
