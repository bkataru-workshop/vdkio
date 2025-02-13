// vdkio/src/codec/h265/parser.rs

use crate::error::Result;
use bytes::{BytesMut, Bytes, BufMut}; // Imported BufMut
use log;

use super::types::{NALUnit, NALUnitType, SPSInfo, PPSInfo, VPSInfo};
use crate::utils::bits::BitReader;

#[derive(Debug)]
pub struct H265Parser {
    sps: Option<SPSInfo>,
    pps: Option<PPSInfo>,
    vps: Option<VPSInfo>, // Added VPS for H265
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
                self.vps = Some(self.parse_vps(&data[1..])?);
            }
            NALUnitType::Sps => {
                self.sps = Some(self.parse_sps(&data[1..])?);
            }
            NALUnitType::Pps => {
                self.pps = Some(self.parse_pps(&data[1..])?);
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

    pub fn is_keyframe(&self, _nalu: &NALUnit) -> bool {
        // Placeholder implementation
        false // Default to false for now
    }

    fn parse_sps(&mut self, data: &[u8]) -> Result<SPSInfo> {
        log::info!("Starting H265 SPS parsing");
        let mut reader = BitReader::new(data);

        let profile_tier_level = reader.read_bits(32)?; 
        log::info!("profile_tier_level: {:?}", profile_tier_level);

        let sps_id = reader.read_golomb()?; 
        log::info!("sps_id: {:?}", sps_id);

        let chroma_format_idc = reader.read_golomb()?;
        log::info!("chroma_format_idc: {:?}", chroma_format_idc);

        let pic_width_max_in_luma_samples = reader.read_golomb()?;
        log::info!("pic_width_max_in_luma_samples: {:?}", pic_width_max_in_luma_samples);

        let pic_height_max_in_luma_samples = reader.read_golomb()?;
        log::info!("pic_height_max_in_luma_samples: {:?}", pic_height_max_in_luma_samples);

        Ok(SPSInfo {
            sps_id: sps_id as u32,
            profile_tier_level,
            chroma_format_idc: chroma_format_idc as u32,
            pic_width_max_in_luma_samples: pic_width_max_in_luma_samples as u32,
            pic_height_max_in_luma_samples: pic_height_max_in_luma_samples as u32,
        })
    }

fn parse_pps(&mut self, _data: &[u8]) -> Result<PPSInfo> {
        // Placeholder implementation
        Err(crate::error::VdkError::Codec("PPS parsing not yet implemented".to_string()))
    }

    fn parse_vps(&mut self, _data: &[u8]) -> Result<VPSInfo> {
        // Placeholder implementation
        Err(crate::error::VdkError::Codec("VPS parsing not yet implemented".to_string()))
    }
}
