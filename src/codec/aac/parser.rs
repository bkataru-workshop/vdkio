use super::types::{AACConfig, AACFrame, ADTSHeader, ProfileType};
use crate::utils::BitReader;
use crate::{Result, VdkError};

#[derive(Debug)]
pub struct AACParser {
    config: Option<AACConfig>,
}

impl AACParser {
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn parse_frame(&mut self, data: &[u8]) -> Result<AACFrame> {
        // Try to parse as ADTS frame first
        if data.len() >= 7 {
            // Minimum ADTS header size
            if let Ok(header) = self.parse_adts_header(&data[..7]) {
                if header.sync_word_valid() {
                    let frame_data = if header.frame_length as usize <= data.len() {
                        &data[7..header.frame_length as usize]
                    } else {
                        &data[7..]
                    };
                    let config = AACConfig {
                        profile: header.profile,
                        sample_rate_index: header.sample_rate_index,
                        channel_configuration: header.channel_configuration,
                        frame_length: 1024, // AAC default frame length
                    };
                    self.config = Some(config.clone());
                    return Ok(AACFrame::new(config, frame_data.to_vec()));
                }
            }
        }

        // If not ADTS, try to use existing config
        if let Some(config) = &self.config {
            Ok(AACFrame::new(config.clone(), data.to_vec()))
        } else {
            Err(VdkError::Parser(
                "No AAC configuration available and data is not in ADTS format".into(),
            ))
        }
    }

    pub fn parse_adts_header(&mut self, data: &[u8]) -> Result<ADTSHeader> {
        if data.len() < 7 {
            return Err(VdkError::Parser("ADTS header too short".into()));
        }

        let mut reader = BitReader::new(data);

        let sync_word = reader.read_bits(12)?;
        if sync_word != 0xFFF {
            return Err(VdkError::Parser("Invalid ADTS sync word".into()));
        }

        let id = reader.read_bits(1)? as u8;
        let layer = reader.read_bits(2)? as u8;
        let protection_absent = reader.read_bits(1)? == 1;

        let profile_raw = reader.read_bits(2)? as u8;
        let profile = ProfileType::from(profile_raw);

        let sample_rate_index = reader.read_bits(4)? as u8;
        let private_bit = reader.read_bits(1)? == 1;
        let channel_configuration = reader.read_bits(3)? as u8;
        let original_copy = reader.read_bits(1)? == 1;
        let home = reader.read_bits(1)? == 1;

        let copyright_id_bit = reader.read_bits(1)? == 1;
        let copyright_id_start = reader.read_bits(1)? == 1;
        let frame_length = reader.read_bits(13).map(|v| v as u16)?;
        let buffer_fullness = reader.read_bits(11)? as u16;
        let number_of_raw_blocks = reader.read_bits(2)? as u8;

        Ok(ADTSHeader {
            sync_word,
            id,
            layer,
            protection_absent,
            profile,
            sample_rate_index,
            private_bit,
            channel_configuration,
            original_copy,
            home,
            copyright_id_bit,
            copyright_id_start,
            frame_length,
            buffer_fullness,
            number_of_raw_blocks,
        })
    }

    pub fn set_config(&mut self, config: AACConfig) {
        self.config = Some(config);
    }

    pub fn config(&self) -> Option<&AACConfig> {
        self.config.as_ref()
    }
}

impl Default for AACParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_adts_header() {
        // ADTS header for AAC-LC, 44.1kHz, stereo
        let data = vec![
            0xFF, 0xF1, // Sync word + ID(0) + Layer(0) + Protection(1)
            0x50, // Profile(1=LC) + SampleRate(4=44.1) + Private(0)
            0x80, // Channel(2=stereo) + Original(0) + Home(0) + ...
            0x43, 0x80, // Frame length = 1024
            0x00, // Buffer fullness + blocks
        ];

        let mut parser = AACParser::new();
        let header = parser.parse_adts_header(&data).unwrap();

        assert!(header.sync_word_valid());
        assert_eq!(header.profile as u8, ProfileType::LC as u8);
        assert_eq!(header.sample_rate_index, 4);
        assert_eq!(header.channel_configuration, 2);
    }

    #[test]
    fn test_parse_frame() {
        // Same ADTS header as above + some dummy frame data
        let mut data = vec![0xFF, 0xF1, 0x50, 0x80, 0x43, 0x80, 0x00];
        // Add some dummy frame data
        data.extend_from_slice(&[1, 2, 3, 4]);

        let mut parser = AACParser::new();
        let frame = parser.parse_frame(&data).unwrap();

        assert_eq!(frame.config.profile as u8, ProfileType::LC as u8);
        assert_eq!(frame.config.sample_rate_index, 4);
        assert_eq!(frame.config.channel_configuration, 2);
        assert_eq!(frame.data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_invalid_sync_word() {
        let data = vec![0x00, 0x00, 0x50, 0x80, 0x43, 0x80, 0x00];
        let mut parser = AACParser::new();
        assert!(parser.parse_adts_header(&data).is_err());
    }
}
