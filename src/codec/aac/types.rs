/// AAC Profile types as defined in ISO/IEC 13818-7 (MPEG-2 AAC) and ISO/IEC 14496-3 (MPEG-4 AAC)
#[derive(Debug, Clone, Copy)]
pub enum ProfileType {
    /// Main profile - most complete but computationally intensive
    Main = 0,
    /// Low Complexity profile - most widely used
    LC = 1,
    /// Scalable Sampling Rate profile
    SSR = 2,
    /// Long Term Prediction profile
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

/// Configuration parameters for AAC audio streams
#[derive(Debug, Clone)]
pub struct AACConfig {
    /// AAC profile type (Main, LC, SSR, or LTP)
    pub profile: ProfileType,
    /// Index representing sample rate (0=96000 Hz, 1=88200 Hz, etc.)
    pub sample_rate_index: u8,
    /// Number of channels (1=mono, 2=stereo, etc.)
    pub channel_configuration: u8,
    /// Number of samples per frame (typically 1024)
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

/// Audio Data Transport Stream (ADTS) header structure as defined in ISO/IEC 13818-7
/// Contains frame synchronization and configuration information for AAC audio frames
#[derive(Debug)]
pub struct ADTSHeader {
    /// Synchronization word, always 0xFFF (12 bits)
    pub sync_word: u32,
    /// MPEG version ID, 0=MPEG-4, 1=MPEG-2 (1 bit)
    pub id: u8,
    /// MPEG layer, always 0 for AAC (2 bits)
    pub layer: u8,
    /// CRC protection flag, true if CRC is absent (1 bit)
    pub protection_absent: bool,
    /// AAC profile type (2 bits)
    pub profile: ProfileType,
    /// Sample rate index (4 bits)
    pub sample_rate_index: u8,
    /// Private bit, can be used freely (1 bit)
    pub private_bit: bool,
    /// Channel configuration (3 bits)
    pub channel_configuration: u8,
    /// Original/copy flag (1 bit)
    pub original_copy: bool,
    /// Home flag (1 bit)
    pub home: bool,
    /// Copyright ID bit (1 bit)
    pub copyright_id_bit: bool,
    /// Copyright ID start bit (1 bit)
    pub copyright_id_start: bool,
    /// Length of frame including header in bytes (13 bits)
    pub frame_length: u16,
    /// Buffer fullness value (11 bits)
    pub buffer_fullness: u16,
    /// Number of AAC frames in ADTS frame minus 1 (2 bits)
    pub number_of_raw_blocks: u8,
}

impl ADTSHeader {
    /// Checks if the sync word is valid (equals 0xFFF)
    ///
    /// # Returns
    ///
    /// `true` if the sync word is valid, `false` otherwise
    pub fn sync_word_valid(&self) -> bool {
        self.sync_word == 0xFFF
    }

    /// Gets the actual sample rate in Hz from the sample rate index
    ///
    /// # Returns
    ///
    /// * `Some(rate)` - The sample rate in Hz if the index is valid (96000, 88200, etc.)
    /// * `None` - If the sample rate index is invalid
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

    /// Converts the ADTS header to its binary representation following ISO/IEC 13818-7
    ///
    /// Creates a 7-byte array containing the ADTS header fields packed according to the specification.
    /// Each field is positioned at its correct bit location within the header.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - A 7-byte vector containing the packed ADTS header
    /// * `Err(_)` - If there was an error during conversion
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

/// An AAC frame containing configuration and audio data
///
/// Represents a complete AAC audio frame that can be decoded to produce audio samples.
/// Each frame contains its own configuration to support dynamic changes in audio format.
#[derive(Debug, Clone)]
pub struct AACFrame {
    /// Configuration parameters for this frame including profile, sample rate, and channels
    pub config: AACConfig,
    /// Raw AAC frame data (excluding ADTS header if present)
    pub data: Vec<u8>,
}

impl AACFrame {
    /// Creates a new AAC frame with the given configuration and data
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration parameters for this frame
    /// * `data` - Raw AAC frame data (should not include ADTS header)
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
