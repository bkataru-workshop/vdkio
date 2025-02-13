#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_bits() {
        let data = &[0b10110011, 0b01011010];
        let mut reader = BitReader::new(data);

        assert_eq!(reader.read_bits(3).unwrap(), 0b101);
        assert_eq!(reader.read_bits(5).unwrap(), 0b10011);
        assert_eq!(reader.read_bits(4).unwrap(), 0b0101);
    }

    #[test]
    fn test_read_golomb() {
        // Golomb code examples:
        // 1 => 0 (1 in binary)
        // 010 => 1 (2 in binary)
        // 011 => 2 (3 in binary)
        let data = &[0b10100110];  // Contains multiple golomb codes
        let mut reader = BitReader::new(data);

        assert_eq!(reader.read_golomb().unwrap(), 0);  // reads '1'
        assert_eq!(reader.read_golomb().unwrap(), 1);  // reads '010'
    }

    #[test]
    fn test_read_signed_golomb() {
        // Signed Golomb examples:
        // 1 => 0
        // 010 => 1
        // 011 => -1
        // 00100 => 2
        // 00101 => -2
        let data = &[0b10110001, 0b00000000];
        let mut reader = BitReader::new(data);

        assert_eq!(reader.read_signed_golomb().unwrap(), 0);
        assert_eq!(reader.read_signed_golomb().unwrap(), -1);
    }

    #[test]
    fn test_skip_bits() {
        let data = &[0b10110011, 0b01011010];
        let mut reader = BitReader::new(data);

        reader.skip_bits(3).unwrap();
        assert_eq!(reader.read_bits(5).unwrap(), 0b10011);
    }

    #[test]
    fn test_align_to_byte() {
        let data = &[0b10110011, 0b01011010];
        let mut reader = BitReader::new(data);

        reader.read_bits(3).unwrap();
        reader.align_to_byte();
        assert_eq!(reader.read_bits(8).unwrap(), 0b01011010);
    }

    #[test]
    fn test_error_handling() {
        let data = &[0b10110011];
        let mut reader = BitReader::new(data);

        // Try to read more bits than available
        reader.read_bits(6).unwrap();
        assert!(reader.read_bits(8).is_err());

        // Try to read more than 32 bits
        let mut reader = BitReader::new(data);
        assert!(reader.read_bits(33).is_err());
    }
}
