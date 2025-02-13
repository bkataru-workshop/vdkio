#[derive(Debug, Clone, Copy)]
pub enum ProfileType {
    Main = 0,
    LC = 1,
    SSR = 2,
    LTP = 3,
}

impl From<u8> for ProfileType {
    fn from(value: u8) -> Self {
        match value {
            0 => ProfileType::Main,
            1 => ProfileType::LC,
            2 => ProfileType::SSR,
            3 => ProfileType::LTP,
            _ => ProfileType::LC, // Default to LC for unknown profiles
        }
    }
}

#[derive(Debug, Clone)]
pub struct AACConfig {
    pub profile: ProfileType,
    pub sample_rate_index: u8,
    pub channel_configuration: u8,
    pub frame_length: u16,
}

impl Default for AACConfig {
    fn default() -> Self {
        Self {
            profile: ProfileType::LC,
            sample_rate_index: 4, // 44100 Hz
            channel_configuration: 2, // Stereo
            frame_length: 1024,
        }
    }
}

#[derive(Debug)]
pub struct ADTSHeader {
    pub sync_word: u32,          // 12 bits
    pub id: u8,                  // 1 bit, 0=MPEG-4, 1=MPEG-2
    pub layer: u8,               // 2 bits
    pub protection_absent: bool,  // 1 bit
    pub profile: ProfileType,    // 2 bits
    pub sample_rate_index: u8,   // 4 bits
    pub private_bit: bool,       // 1 bit
    pub channel_configuration: u8,// 3 bits
    pub original_copy: bool,     // 1 bit
    pub home: bool,              // 1 bit
    pub copyright_id_bit: bool,  // 1 bit
    pub copyright_id_start: bool,// 1 bit
    pub frame_length: u16,       // 13 bits
    pub buffer_fullness: u16,    // 11 bits
    pub number_of_raw_blocks: u8,// 2 bits
}

impl ADTSHeader {
    pub fn sync_word_valid(&self) -> bool {
        self.sync_word == 0xFFF
    }

    pub fn sample_rate(&self) -> Option<u32> {
        match self.sample_rate_index {
            0 => Some(96000),
            1 => Some(88200),
            2 => Some(64000),
            3 => Some(48000),
            4 => Some(44100),
            5 => Some(32000),
            6 => Some(24000),
            7 => Some(22050),
            8 => Some(16000),
            9 => Some(12000),
            10 => Some(11025),
            11 => Some(8000),
            12 => Some(7350),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AACFrame {
    pub config: AACConfig,
    pub data: Vec<u8>,
}

impl AACFrame {
    pub fn new(config: AACConfig, data: Vec<u8>) -> Self {
        Self { config, data }
    }
}
