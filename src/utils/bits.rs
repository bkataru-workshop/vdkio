use crate::Result;
use crate::VdkError;

pub struct BitReader<'a> {
    data: &'a [u8],
    position: usize,
    bit_position: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            bit_position: 0,
        }
    }

    pub fn read_bits(&mut self, count: u8) -> Result<u32> {
        log::info!("BitReader::read_bits: count = {}, position = {}, bit_position = {}", count, self.position, self.bit_position);
        if count > 32 {
            return Err(VdkError::InvalidData("Cannot read more than 32 bits".into()));
        }

        let mut result = 0u32;
        let mut bits_left = count;

        while bits_left > 0 {
            if self.position >= self.data.len() {
                return Err(VdkError::InvalidData("Reached end of data".into()));
            }

            let byte = self.data[self.position];
            let bits_in_byte = 8 - self.bit_position;
            let bits_to_read = bits_in_byte.min(bits_left);
            
            let mask = ((1u16 << bits_to_read) - 1) as u8;
            let shifted = (byte >> (8 - self.bit_position - bits_to_read)) & mask;
            
            result = (result << bits_to_read) | shifted as u32;
            
            self.bit_position += bits_to_read;
            if self.bit_position >= 8 {
                self.position += 1;
                self.bit_position = 0;
            }
            
            bits_left -= bits_to_read;
        }
        log::info!("BitReader::read_bits: result = {}", result);

        Ok(result)
    }

    pub fn read_golomb(&mut self) -> Result<u32> {
        let mut leading_zeros = 0u32;

        // Count leading zeros
        while self.read_bits(1)? == 0 {
            leading_zeros += 1;
        }

        if leading_zeros == 0 {
            return Ok(0);
        }

        let remaining = self.read_bits(leading_zeros as u8)?;
        Ok((1 << leading_zeros) - 1 + remaining)
    }

    pub fn read_signed_golomb(&mut self) -> Result<i32> {
        let code = self.read_golomb()?;
        let sign = (code & 1) == 1;
        let abs = (code + 1) >> 1;
        Ok(if sign { abs as i32 } else { -(abs as i32) })
    }

    pub fn skip_bits(&mut self, count: u32) -> Result<()> {
        let new_pos = self.position * 8 + self.bit_position as usize + count as usize;
        self.position = new_pos / 8;
        self.bit_position = (new_pos % 8) as u8;

        if self.position > self.data.len() {
            return Err(VdkError::InvalidData("Attempted to skip past end of data".into()));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn align_to_byte(&mut self) {
        if self.bit_position > 0 {
            self.position += 1;
            self.bit_position = 0;
        }
    }
}
