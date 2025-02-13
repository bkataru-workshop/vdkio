// vdkio/src/codec/h265/types.rs

#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum NALUnitType {
    TrailN = 0,
    TrailR = 1,
    TsaN = 2,
    TsaR = 3,
    StsaN = 4,
    StsaR = 5,
    RadlN = 6,
    RadlR = 7,
    RaslN = 8,
    RaslR = 9,
    ReservedVcl4 = 10,
    ReservedVcl5 = 11,
    ReservedVcl6 = 12,
    IdrWRadl = 19,
    IdrNLp = 20,
    BlaWLp = 21,
    BlaWRadl = 22,
    BlaNLp = 23,
    SubLayerNonVclBase = 24, // Changed from 32 to avoid duplicate discriminant
    Vps = 33,
    Sps = 34,
    Pps = 35,
    Aud = 36,
    Eos = 37,
    Eob = 38,
    PrefixSei = 39,
    SuffixSei = 40,
    ReservedNvcl26 = 26,
    ReservedNvcl27 = 27,
    ReservedNvcl28 = 28,
    ReservedNvcl29 = 29,
    ReservedNvcl30 = 30,
    ReservedNvcl31 = 31,
    Unspecified(u8),
}

impl NALUnitType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => NALUnitType::TrailN,
            1 => NALUnitType::TrailR,
            2 => NALUnitType::TsaN,
            3 => NALUnitType::TsaR,
            4 => NALUnitType::StsaN,
            5 => NALUnitType::StsaR,
            6 => NALUnitType::RadlN,
            7 => NALUnitType::RadlR,
            8 => NALUnitType::RaslN,
            9 => NALUnitType::RaslR,
            10 => NALUnitType::ReservedVcl4,
            11 => NALUnitType::ReservedVcl5,
            12 => NALUnitType::ReservedVcl6,
            19 => NALUnitType::IdrWRadl,
            20 => NALUnitType::IdrNLp,
            21 => NALUnitType::BlaWLp,
            22 => NALUnitType::BlaWRadl,
            23 => NALUnitType::BlaNLp,
            24 => NALUnitType::SubLayerNonVclBase,
            33 => NALUnitType::Vps,
            34 => NALUnitType::Sps,
            35 => NALUnitType::Pps,
            36 => NALUnitType::Aud,
            37 => NALUnitType::Eos,
            38 => NALUnitType::Eob,
            39 => NALUnitType::PrefixSei,
            40 => NALUnitType::SuffixSei,
            26 => NALUnitType::ReservedNvcl26,
            27 => NALUnitType::ReservedNvcl27,
            28 => NALUnitType::ReservedNvcl28,
            29 => NALUnitType::ReservedNvcl29,
            30 => NALUnitType::ReservedNvcl30,
            31 => NALUnitType::ReservedNvcl31,
            _ => NALUnitType::Unspecified(value),
        }
    }
}

use bytes::Bytes;

#[derive(Debug)]
pub struct NALUnit {
    pub nal_type: NALUnitType, // Added nal_type field of type NALUnitType
    pub data: Bytes,
}

impl NALUnit {
    pub fn new(data: Bytes) -> Self {
        let nal_type_byte = (data[0] >> 1) & 0x3F; // Extract nal_unit_type from first byte (6 bits for H265)
        let nal_type = NALUnitType::from_u8(nal_type_byte);
        NALUnit {
            nal_type,
            data,
        }
    }
}


#[derive(Debug)]
pub struct SPSInfo {
    pub sps_id: u32,
    pub profile_tier_level: u32,
    pub chroma_format_idc: u32,
    pub pic_width_max_in_luma_samples: u32,
    pub pic_height_max_in_luma_samples: u32,
    // ... more SPS parameters to be added
}

#[derive(Debug)]
pub struct PPSInfo {}

#[derive(Debug)]
pub struct VPSInfo {}
