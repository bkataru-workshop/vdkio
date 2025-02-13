use bytes::{Bytes, BytesMut, BufMut};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::Result;
use crate::utils::BitReader;
use super::types::{NALUnit, NALUnitType, SPSInfo, PPSInfo};

#[derive(Debug)]
struct ParserState {
    sps: Option<SPSInfo>,
    pps: Option<PPSInfo>,
}

#[derive(Debug)]
pub struct H264Parser {
    state: Arc<Mutex<ParserState>>,
    buffer: BytesMut,
}

impl H264Parser {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ParserState {
                sps: None,
                pps: None,
            })),
            buffer: BytesMut::new(),
        }
    }

    pub fn parse_nalu(&mut self, data: &[u8]) -> Result<NALUnit> {
        let data = self.remove_emulation_prevention(data);
        let data = Bytes::from(data);
        let nalu = NALUnit::new(data.clone());

        match NALUnitType::from(nalu.nal_type) {
            NALUnitType::SPS => {
                let mut state = self.state.lock();
                state.sps = Some(self.parse_sps(&data[1..])?);
            }
            NALUnitType::PPS => {
                let mut state = self.state.lock();
                state.pps = Some(self.parse_pps(&data[1..])?);
            }
            _ => {}
        }

        Ok(nalu)
    }

    pub fn remove_emulation_prevention(&mut self, data: &[u8]) -> Vec<u8> {
        self.buffer.clear();
        let mut i = 0;

        while i < data.len() {
            if i + 2 < data.len() 
                && data[i] == 0x00 
                && data[i + 1] == 0x00 
                && data[i + 2] == 0x03 {
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

    fn parse_sps(&self, data: &[u8]) -> Result<SPSInfo> {
        log::info!("Starting SPS parsing, data: {:?}", data);
        let mut reader = BitReader::new(data);
        
        let profile_idc = reader.read_bits(8)? as u8;
        log::info!("profile_idc: {:?}", profile_idc);
        reader.skip_bits(16)?; // constraint flags and reserved bits
        let level_idc = reader.read_bits(8)? as u8;
        log::info!("level_idc: {:?}", level_idc);
        
        reader.read_golomb()?; // seq_parameter_set_id
        log::info!("seq_parameter_set_id parsed");
        
        // Skip chroma format related fields for high profiles
        if profile_idc == 100 || profile_idc == 110 || profile_idc == 122 || profile_idc == 244 || 
           profile_idc == 44 || profile_idc == 83 || profile_idc == 86 || profile_idc == 118 || 
           profile_idc == 128 || profile_idc == 138 {
            let chroma_format_idc = reader.read_golomb()?;
            if chroma_format_idc == 3 {
                reader.read_bits(1)?; // separate_colour_plane_flag
            }
            reader.read_golomb()?; // bit_depth_luma_minus8
            reader.read_golomb()?; // bit_depth_chroma_minus8
            reader.read_bits(1)?; // qpprime_y_zero_transform_bypass_flag
            
            // scaling matrices
            let scaling_matrix_present = reader.read_bits(1)?;
            if scaling_matrix_present == 1 {
                let count = if chroma_format_idc != 3 { 8 } else { 12 };
                for _ in 0..count {
                    let scaling_list_present = reader.read_bits(1)?;
                    if scaling_list_present == 1 {
                        if count < 6 {
                            self.skip_scaling_list(&mut reader, 16)?;
                        } else {
                            self.skip_scaling_list(&mut reader, 64)?;
                        }
                    }
                }
            }
        }
        
        reader.read_golomb()?; // log2_max_frame_num_minus4
        let pic_order_cnt_type = reader.read_golomb()?;
        
        if pic_order_cnt_type == 0 {
            reader.read_golomb()?; // log2_max_pic_order_cnt_lsb_minus4
        } else if pic_order_cnt_type == 1 {
            reader.read_bits(1)?; // delta_pic_order_always_zero_flag
            reader.read_signed_golomb()?; // offset_for_non_ref_pic
            reader.read_signed_golomb()?; // offset_for_top_to_bottom_field
            let num_ref_frames_in_pic_order_cnt_cycle = reader.read_golomb()?;
            for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
                reader.read_signed_golomb()?;
            }
        }
        
        reader.read_golomb()?; // max_num_ref_frames
        reader.read_bits(1)?; // gaps_in_frame_num_value_allowed_flag
        
        let pic_width_in_mbs = reader.read_golomb()? + 1;
        let pic_height_in_map_units = reader.read_golomb()? + 1;
        let frame_mbs_only_flag = reader.read_bits(1)?;
        
        let width = pic_width_in_mbs * 16;
        let height = (2 - frame_mbs_only_flag) * pic_height_in_map_units * 16;
        
        Ok(SPSInfo {
            profile_idc,
            level_idc,
            width: width as u32,
            height: height as u32,
            frame_rate: None, // We don't parse VUI parameters yet
        })
    }

    fn parse_pps(&self, data: &[u8]) -> Result<PPSInfo> {
        let mut reader = BitReader::new(data);
        
        let pic_parameter_set_id = reader.read_golomb()?;
        let seq_parameter_set_id = reader.read_golomb()?;
        let entropy_coding_mode_flag = reader.read_bits(1)? == 1;
        
        Ok(PPSInfo {
            pic_parameter_set_id,
            seq_parameter_set_id,
            entropy_coding_mode_flag,
        })
    }

    fn skip_scaling_list(&self, reader: &mut BitReader, size: usize) -> Result<()> {
        let mut last_scale = 8;
        let mut next_scale = 8;
        
        for _ in 0..size {
            if next_scale != 0 {
                let delta_scale = reader.read_signed_golomb()?;
                next_scale = (last_scale + delta_scale + 256) % 256;
            }
            last_scale = if next_scale == 0 { last_scale } else { next_scale };
        }
        
        Ok(())
    }

    pub fn dimensions(&self) -> Option<(u32, u32)> {
        let state = self.state.lock();
        state.sps.as_ref().map(|sps| (sps.width, sps.height))
    }

    pub fn is_keyframe(&self, nalu: &NALUnit) -> bool {
        nalu.is_keyframe()
    }
}

impl Default for H264Parser {
    fn default() -> Self {
        Self::new()
    }
}
