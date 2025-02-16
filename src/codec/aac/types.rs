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
            sample_rate_index: 4,     // 44100 Hz
            channel_configuration: 2, // Stereo
            frame_length: 1024,
        }
    }
}

#[derive(Debug)]
pub struct ADTSHeader {
    pub sync_word: u32,            // 12 bits
    pub id: u8,                    // 1 bit, 0=MPEG-4, 1=MPEG-2
    pub layer: u8,                 // 2 bits
    pub protection_absent: bool,   // 1 bit
    pub profile: ProfileType,      // 2 bits
    pub sample_rate_index: u8,     // 4 bits
    pub private_bit: bool,         // 1 bit
    pub channel_configuration: u8, // 3 bits
    pub original_copy: bool,       // 1 bit
    pub home: bool,                // 1 bit
    pub copyright_id_bit: bool,    // 1 bit
    pub copyright_id_start: bool,  // 1 bit
    pub frame_length: u16,         // 13 bits
    pub buffer_fullness: u16,      // 11 bits
    pub number_of_raw_blocks: u8,  // 2 bits
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

    pub fn to_bytes(&self) -> crate::Result<Vec<u8>> {
        let mut bytes = vec![0u8; 7]; // ADTS header is 7 bytes

        // First byte: sync word (first 8 bits)
        bytes[0] = (self.sync_word >> 4) as u8;

        // Second byte: sync word (last 4 bits) + id (1 bit) + layer (2 bits) + protection_absent (1 bit)
        bytes[1] = ((self.sync_word & 0xF) << 4) as u8
            | ((self.id & 0x1) << 3)
            | ((self.layer & 0x3) << 1)
            | (self.protection_absent as u8);

        // Third byte: profile (2 bits) + sample_rate_index (4 bits) + private_bit (1 bit) + channel_configuration (1 bit of 3)
        bytes[2] = ((self.profile as u8) << 6)
            | ((self.sample_rate_index & 0xF) << 2)
            | ((self.private_bit as u8) << 1)
            | ((self.channel_configuration >> 2) & 0x1);

        // Fourth byte: channel_configuration (2 bits) + original_copy (1 bit) + home (1 bit) + copyright_id_bit (1 bit) +
        // copyright_id_start (1 bit) + frame_length (2 bits of 13)
        bytes[3] = ((self.channel_configuration & 0x3) << 6)
            | ((self.original_copy as u8) << 5)
            | ((self.home as u8) << 4)
            | ((self.copyright_id_bit as u8) << 3)
            | ((self.copyright_id_start as u8) << 2)
            | ((self.frame_length >> 11) & 0x3) as u8;

        // Fifth byte: frame_length (8 bits of remaining 11)
        bytes[4] = ((self.frame_length >> 3) & 0xFF) as u8;

        // Sixth byte: frame_length (3 bits) + buffer_fullness (5 bits of 11)
        bytes[5] =
            ((self.frame_length & 0x7) << 5) as u8 | ((self.buffer_fullness >> 6) & 0x1F) as u8;

        // Seventh byte: buffer_fullness (6 bits) + number_of_raw_blocks (2 bits)
        bytes[6] = ((self.buffer_fullness & 0x3F) << 2) as u8 | (self.number_of_raw_blocks & 0x3);

        Ok(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adts_header_to_bytes() {
        let header = ADTSHeader {
            sync_word: 0xFFF,
            id: 0,
            layer: 0,
            protection_absent: true,
            profile: ProfileType::LC,
            sample_rate_index: 4, // 44.1kHz
            private_bit: false,
            channel_configuration: 2, // Stereo
            original_copy: false,
            home: false,
            copyright_id_bit: false,
            copyright_id_start: false,
            frame_length: 1031, // Example frame length
            buffer_fullness: 0x7FF,
            number_of_raw_blocks: 0,
        };

        let bytes = header.to_bytes().unwrap();
        assert_eq!(bytes.len(), 7);
        assert_eq!(bytes[0], 0xFF); // First byte of sync word
        assert_eq!(bytes[1] & 0xF0, 0xF0); // Last 4 bits of sync word
    }
}
