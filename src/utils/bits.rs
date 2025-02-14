use crate::error::{Result, VdkError};

/// A bit-level reader for parsing binary data streams.
///
/// Implements H.264/H.265 style bit reading operations including:
/// - Reading individual bits
/// - Reading multiple bits as numbers
/// - Reading exponential Golomb codes (ue(v))
/// - Reading signed exponential Golomb codes (se(v))
///
/// Example:
/// ```
/// use vdkio::utils::BitReader;
/// 
/// let data = [0b10110011];
/// let mut reader = BitReader::new(&data);
/// 
/// assert_eq!(reader.read_bit().unwrap(), true);   // 1
/// assert_eq!(reader.read_bits(3).unwrap(), 0b011); // 011
/// ```
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_offset: usize,
    bit_offset: u8,
}

impl<'a> BitReader<'a> {
    /// Creates a new BitReader from a byte slice
    pub fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            byte_offset: 0,
            bit_offset: 0,
        }
    }

    /// Reads a single bit from the stream.
    /// Returns true for 1, false for 0.
    ///
    /// Returns error if end of data is reached.
    pub fn read_bit(&mut self) -> Result<bool> {
        if self.byte_offset >= self.data.len() {
            return Err(VdkError::Codec("Reached end of data".into()));
        }

        let bit = (self.data[self.byte_offset] >> (7 - self.bit_offset)) & 1;
        self.bit_offset += 1;

        if self.bit_offset == 8 {
            self.bit_offset = 0;
            self.byte_offset += 1;
        }

        Ok(bit == 1)
    }

    /// Reads n bits and returns them as a number.
    /// The bits are interpreted as big-endian.
    ///
    /// Returns error if n > 32 or end of data is reached.
    pub fn read_bits(&mut self, n: u32) -> Result<u32> {
        if n > 32 {
            return Err(VdkError::Codec("Too many bits requested".into()));
        }

        let mut value = 0u32;
        let n = n as usize;
        
        for i in 0..n {
            let bit = self.read_bit()?;
            if bit {
                value |= 1 << (n - 1 - i);
            }
        }
        
        Ok(value)
    }

    /// Reads an unsigned exponential Golomb code (ue(v)) as specified in H.264/H.265.
    ///
    /// Format:
    /// 1. M leading zeros followed by a 1
    /// 2. M more INFO bits
    /// 3. Value = 2^M + INFO - 1
    ///
    /// Example: "00110" (M=2, INFO=10)
    /// - Count zeros until 1: M=2
    /// - Read 2 more bits: INFO=10=2
    /// - Value = 2^2 + 2 - 1 = 4 + 2 - 1 = 5
    pub fn read_golomb(&mut self) -> Result<u32> {
        let mut leading_zeros = 0;
        while !self.read_bit()? {
            leading_zeros += 1;
            if leading_zeros > 31 {
                return Err(VdkError::Codec("Invalid Golomb code".into()));
            }
        }

        if leading_zeros == 0 {
            return Ok(0);
        }

        let info = self.read_bits(leading_zeros)?;
        Ok((1u32 << leading_zeros) + info - 1)
    }

    /// Reads a signed exponential Golomb code (se(v)) as specified in H.264/H.265.
    ///
    /// The mapping from unsigned (k) to signed is:
    /// - k=0 -> 0
    /// - For k>0:
    ///   * magnitude = (k+1)>>1
    ///   * sign from parity (odd k -> positive, even k -> negative)
    ///
    /// Example: k=4
    /// - magnitude = (4+1)>>1 = 2
    /// - k is even -> negative
    /// - value = -2
    pub fn read_signed_golomb(&mut self) -> Result<i32> {
        let k = self.read_golomb()?;
        if k == 0 {
            return Ok(0);
        }

        let magnitude = ((k + 1) >> 1) as i32;
        let sign = if k & 1 == 1 { 1 } else { -1 };
        Ok(sign * magnitude)
    }

    /// Skips n bits in the stream.
    pub fn skip_bits(&mut self, n: u32) -> Result<()> {
        let n = n as usize;
        for _ in 0..n {
            self.read_bit()?;
        }
        Ok(())
    }

    /// Aligns reader to next byte boundary by skipping remaining bits in current byte.
    pub fn align_byte(&mut self) -> Result<()> {
        if self.bit_offset != 0 {
            self.bit_offset = 0;
            self.byte_offset += 1;
        }
        Ok(())
    }

    /// Returns number of bits available to read.
    pub fn available_bits(&self) -> usize {
        (self.data.len() - self.byte_offset) * 8 - self.bit_offset as usize
    }
}

#[cfg(test)]
mod test_utils {
    // Test utilities for encoding exp-Golomb codes
    
    /// Encodes a single value as exp-Golomb code per H.264/H.265 spec.
    pub fn encode_golomb(value: u32) -> Vec<u8> {
        if value == 0 {
            return vec![0b10000000];
        }

        let leading_zeros = 32 - (value + 1).leading_zeros() - 1;
        let info = value - ((1u32 << leading_zeros) - 1);
        
        let total_bits = (leading_zeros as usize) * 2 + 1;
        let total_bytes = (total_bits + 7) / 8;
        let mut result = vec![0u8; total_bytes];

        let mut bit_pos: usize = 0;
        
        // Write M zeros (already 0)
        bit_pos += leading_zeros as usize;

        // Write 1 marker
        result[bit_pos / 8] |= 1 << (7 - (bit_pos % 8));
        bit_pos += 1;

        // Write INFO bits
        for i in 0..leading_zeros as usize {
            let bit = (info >> (leading_zeros - 1 - i as u32)) & 1;
            if bit == 1 {
                result[bit_pos / 8] |= 1 << (7 - (bit_pos % 8));
            }
            bit_pos += 1;
        }

        result
    }

    /// Encodes multiple values into a single byte array, handling bit packing.
    pub fn encode_multiple_golomb(values: &[u32]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut current_byte = 0u8;
        let mut bits_in_byte: usize = 0;

        for &value in values {
            let encoded = encode_golomb(value);
            
            for &byte in &encoded {
                let remaining = 8 - bits_in_byte;
                current_byte |= (byte >> bits_in_byte) & ((1 << remaining) - 1);
                
                if remaining <= 8 {
                    result.push(current_byte);
                    current_byte = byte << remaining;
                    bits_in_byte = 8 - remaining;
                }
            }
        }

        if bits_in_byte > 0 {
            result.push(current_byte);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::test_utils::*;
    use pretty_assertions::assert_eq;
    use quickcheck_macros::quickcheck;

    #[test]
    fn test_read_bits() {
        // Test case 1: Simple pattern within a byte
        let data = [0b10110011];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bits(3).unwrap(), 0b101);
        assert_eq!(reader.read_bits(5).unwrap(), 0b10011);

        // Test case 2: Cross-byte boundary
        let data = [0b10110011, 0b01011010];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bits(3).unwrap(), 0b101);
        assert_eq!(reader.read_bits(8).unwrap(), 0b10011010);

        // Test case 3: Edge case - reading a full byte
        let data = [0b11111111];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bits(8).unwrap(), 0b11111111);

        // Test case 4: Edge case - reading zero bits
        let data = [0b10101010];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bits(0).unwrap(), 0);

        // Test case 5: Error on too many bits
        let data = [0xFF];
        let mut reader = BitReader::new(&data);
        assert!(reader.read_bits(33).is_err());

        // Test case 6: Cross multiple byte boundaries
        let data = [0b10110011, 0b11001100, 0b10101010];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bits(20).unwrap(), 0b10110011110011001010);
    }

    #[test]
    fn test_read_golomb() {
        // Test known patterns from H.264 spec
        let test_cases = [
            ([0b10000000], 0, "1"),       // zeros=0: 0
            ([0b01000000], 1, "010"),     // zeros=1,INFO=0: 2+0-1=1
            ([0b01100000], 2, "011"),     // zeros=1,INFO=1: 2+1-1=2
            ([0b00100000], 3, "00100"),   // zeros=2,INFO=00: 4+0-1=3
            ([0b00110000], 5, "00110"),   // zeros=2,INFO=10: 4+2-1=5
            ([0b00101000], 4, "00101"),   // zeros=2,INFO=01: 4+1-1=4
            ([0b00111000], 6, "00111"),   // zeros=2,INFO=11: 4+3-1=6
            ([0b00010000], 7, "0001000"), // zeros=3,INFO=000: 8+0-1=7
            ([0b00010010], 8, "0001001"), // zeros=3,INFO=001: 8+1-1=8
        ];

        for (_i, (input, expected, pattern)) in test_cases.iter().enumerate() {
            let mut reader = BitReader::new(input);
            let result = reader.read_golomb().unwrap();
            assert_eq!(result, *expected, "Failed for pattern {}", pattern);

            // Verify our encoder generates same pattern
            let encoded = encode_golomb(*expected);
            assert_eq!(&encoded[..1], input, 
                      "Encoding {} gave wrong pattern", expected);
        }

        // Test error on invalid input
        let data = [0x00]; // All zeros
        let mut reader = BitReader::new(&data);
        assert!(reader.read_golomb().is_err());
    }

    #[test]
    fn test_signed_golomb() {
        // Test k to signed value mapping
        let test_cases = [
            ([0b10000000], 0,  0, "k=0 -> 0"),         // Special case
            ([0b01000000], 1,  1, "k=1 -> +1"),        // Odd -> positive
            ([0b01100000], 2, -1, "k=2 -> -1"),        // Even -> negative
            ([0b00100000], 3,  2, "k=3 -> +2"),        // Odd -> positive
            ([0b00110000], 5, -3, "k=5 -> -3"),        // Odd -> positive
            ([0b00101000], 4,  2, "k=4 -> +2"),        // Even -> negative
            ([0b00111000], 6, -3, "k=6 -> -3"),        // Even -> negative
            ([0b00010000], 7,  4, "k=7 -> +4"),        // Odd -> positive
            ([0b00010010], 8, -4, "k=8 -> -4"),        // Even -> negative
        ];

        for (_i, (input, code, expected, desc)) in test_cases.iter().enumerate() {
            let mut reader = BitReader::new(input);
            let result = reader.read_signed_golomb().unwrap();
            assert_eq!(result, *expected,
                      "Failed for code {} ({})", code, desc);
        }
    }

    #[test]
    fn test_consecutive_golomb() {
        // Test reading multiple consecutive codes
        let values = [3, 5, 1, 0, 4];
        let encoded = encode_multiple_golomb(&values);
        let mut reader = BitReader::new(&encoded);

        for &expected in &values {
            let result = reader.read_golomb().unwrap();
            assert_eq!(result, expected, "Failed reading value {}", expected);
        }
    }

    #[quickcheck]
    fn prop_read_bits_matches_manual(data: Vec<u8>, n: u8) -> bool {
        if data.is_empty() || n > 32 { return true; }

        let mut reader = BitReader::new(&data);
        let n = n % 32; // Keep n in valid range
        
        match reader.read_bits(n as u32) {
            Ok(result) => {
                let mut expected = 0u32;
                for i in 0..n as usize {
                    let byte_idx = i / 8;
                    let bit_idx = 7 - (i % 8);
                    if byte_idx >= data.len() { return true; }
                    let bit = (data[byte_idx] >> bit_idx) & 1;
                    expected |= (bit as u32) << (n - 1 - i as u8);
                }
                result == expected
            }
            Err(_) => true
        }
    }

    #[quickcheck]
    fn prop_golomb_round_trip(values: Vec<u8>) -> bool {
        if values.is_empty() { return true; }
        
        // Encode multiple values
        let values: Vec<u32> = values.into_iter()
            .map(|v| (v as u32) % 256)  // Keep values small
            .collect();
        
        let encoded = encode_multiple_golomb(&values);
        let mut reader = BitReader::new(&encoded);

        // Read back and verify
        for &expected in &values {
            match reader.read_golomb() {
                Ok(decoded) if decoded == expected => continue,
                _ => return false
            }
        }
        true
    }

    #[quickcheck]
    fn prop_signed_mapping(data: Vec<u8>) -> bool {
        if data.is_empty() { return true; }

        let mut reader = BitReader::new(&data);
        match reader.read_signed_golomb() {
            Ok(v) if v == 0 => {
                match reader.read_golomb() {
                    Ok(k) => k == 0,
                    Err(_) => true
                }
            }
            Ok(v) => {
                match reader.read_golomb() {
                    Ok(k) => {
                        if k & 1 == 1 {
                            v > 0  // Odd k -> positive
                        } else {
                            v < 0  // Even k -> negative
                        }
                    }
                    Err(_) => true
                }
            }
            Err(_) => true
        }
    }

    #[test]
    fn test_error_cases() {
        // Test reading past end of data
        let data = [0xFF];
        let mut reader = BitReader::new(&data);
        reader.read_bits(8).unwrap();
        assert!(reader.read_bit().is_err());

        // Test invalid Golomb code (too many zeros)
        let data = vec![0; 5];  // 40 zeros
        let mut reader = BitReader::new(&data);
        assert!(reader.read_golomb().is_err());

        // Test byte alignment
        let data = [0xFF, 0x00];
        let mut reader = BitReader::new(&data);
        reader.read_bits(3).unwrap();
        assert_eq!(reader.bit_offset, 3);
        reader.align_byte().unwrap();
        assert_eq!(reader.bit_offset, 0);
        assert_eq!(reader.byte_offset, 1);
    }
}
