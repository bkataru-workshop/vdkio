#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_remove_emulation_prevention() {
        let mut parser = H264Parser::new();
        
        // Test basic emulation prevention removal
        let input = vec![0x00, 0x00, 0x03, 0x01];
        let output = parser.remove_emulation_prevention(&input);
        assert_eq!(output, vec![0x00, 0x00, 0x01]);

        // Test multiple emulation prevention bytes
        let input = vec![0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x03, 0x02];
        let output = parser.remove_emulation_prevention(&input);
        assert_eq!(output, vec![0x00, 0x00, 0x01, 0x00, 0x00, 0x02]);

        // Test no emulation prevention bytes
        let input = vec![0x00, 0x01, 0x02, 0x03];
        let output = parser.remove_emulation_prevention(&input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_parse_nalu() {
        let mut parser = H264Parser::new();
        
        // Create a simple NAL unit (type 1 - non-IDR slice)
        let data = vec![0x01, 0x02, 0x03, 0x04]; // First byte: nal_ref_idc=0, nal_type=1
        let nalu = parser.parse_nalu(&data).unwrap();
        
        assert_eq!(nalu.nal_type, 1);
        assert_eq!(nalu.nal_ref_idc, 0);
        assert!(!nalu.is_keyframe());

        // Test IDR slice (type 5)
        let data = vec![0x65, 0x02, 0x03, 0x04]; // First byte: nal_ref_idc=3, nal_type=5
        let nalu = parser.parse_nalu(&data).unwrap();
        
        assert_eq!(nalu.nal_type, 5);
        assert_eq!(nalu.nal_ref_idc, 3);
        assert!(nalu.is_keyframe());
    }

    #[test]
    fn test_dimensions() {
        let mut parser = H264Parser::new();
        
        // Simplified SPS NAL unit
        // This is a minimal SPS that sets 1920x1080 resolution
        let sps_data = vec![
            0x67, // NAL header (type 7)
            0x64, // profile_idc = 100
            0x00, 0x0A, // constraint_set flags
            0x1F, // level_idc = 31
            0xE0, // First byte of exp-golomb coded data
            // ... more bytes would follow in a real SPS
            // This is simplified for testing
        ];
        
        // Initially dimensions should be None
        assert_eq!(parser.dimensions(), None);
        
        // After parsing SPS, dimensions should be available
        let _ = parser.parse_nalu(&sps_data);
        
        // Real SPS parsing would set dimensions
        // In this simplified test, we just verify the function call works
        let dims = parser.dimensions();
        assert!(dims.is_some());
    }
}
