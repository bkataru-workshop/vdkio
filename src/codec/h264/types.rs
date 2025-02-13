use bytes::Bytes;

#[derive(Debug)]
pub struct NALUnit {
    pub nal_type: u8,
    pub nal_ref_idc: u8,
    pub data: Bytes,
}

impl NALUnit {
    pub fn new(data: Bytes) -> Self {
        let header = data[0];
        Self {
            nal_type: header & 0x1F,
            nal_ref_idc: (header >> 5) & 0x03,
            data,
        }
    }

    pub fn is_keyframe(&self) -> bool {
        self.nal_type == 5 || self.nal_type == 7 || self.nal_type == 8
    }
}

#[derive(Debug, Default)]
pub struct SPSInfo {
    pub profile_idc: u8,
    pub level_idc: u8,
    pub width: u32,
    pub height: u32,
    pub frame_rate: Option<f32>,
}

#[derive(Debug, Default)]
pub struct PPSInfo {
    pub pic_parameter_set_id: u32,
    pub seq_parameter_set_id: u32,
    pub entropy_coding_mode_flag: bool,
}

#[derive(Debug)]
pub enum NALUnitType {
    Unspecified = 0,
    CodedSliceNonIDR = 1,
    CodedSliceDataPartitionA = 2,
    CodedSliceDataPartitionB = 3,
    CodedSliceDataPartitionC = 4,
    CodedSliceIDR = 5,
    SEI = 6,
    SPS = 7,
    PPS = 8,
    AccessUnitDelimiter = 9,
    EndOfSequence = 10,
    EndOfStream = 11,
    FillerData = 12,
}

impl From<u8> for NALUnitType {
    fn from(value: u8) -> Self {
        match value {
            1 => NALUnitType::CodedSliceNonIDR,
            2 => NALUnitType::CodedSliceDataPartitionA,
            3 => NALUnitType::CodedSliceDataPartitionB,
            4 => NALUnitType::CodedSliceDataPartitionC,
            5 => NALUnitType::CodedSliceIDR,
            6 => NALUnitType::SEI,
            7 => NALUnitType::SPS,
            8 => NALUnitType::PPS,
            9 => NALUnitType::AccessUnitDelimiter,
            10 => NALUnitType::EndOfSequence,
            11 => NALUnitType::EndOfStream,
            12 => NALUnitType::FillerData,
            _ => NALUnitType::Unspecified,
        }
    }
}
