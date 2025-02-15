use crate::error::{Result, VdkError};
use bytes::Bytes;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NALType {
    Unknown = 0,
    NonIDR = 1,
    IDR = 5,
    SEI = 6,
    SPS = 7,
    PPS = 8,
}

impl NALType {
    pub fn from_u8(val: u8) -> Result<Self> {
        match val & 0x1F {
            0 => Ok(NALType::Unknown),
            1 => Ok(NALType::NonIDR),
            5 => Ok(NALType::IDR),
            6 => Ok(NALType::SEI),
            7 => Ok(NALType::SPS),
            8 => Ok(NALType::PPS),
            _ => Err(VdkError::Codec("Unknown NAL type".into())),
        }
    }
}

#[derive(Debug)]
pub struct NALUnit<'a> {
    pub data: &'a [u8],
}

impl<'a> NALUnit<'a> {
    pub fn find_units(data: &'a [u8]) -> Result<Vec<NALUnit<'a>>> {
        let mut units = Vec::new();
        let mut start = 0;

        // Find start codes
        for i in 0..data.len() - 3 {
            if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
                if start > 0 && start < i {
                    units.push(NALUnit { data: &data[start..i] });
                }
                start = i + 3;
            }
            if i > 0 && data[i - 1] == 0 && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
                if start > 0 && start < i - 1 {
                    units.push(NALUnit { data: &data[start..(i - 1)] });
                }
                start = i + 3;
            }
        }

        // Add final unit
        if start > 0 && start < data.len() {
            units.push(NALUnit { data: &data[start..] });
        }

        Ok(units)
    }

    pub fn header(&self) -> Result<u8> {
        self.data.first()
            .copied()
            .ok_or_else(|| VdkError::Codec("Empty NAL unit".into()))
    }

    pub fn nal_type(&self) -> Result<NALType> {
        let header = self.header()?;
        NALType::from_u8(header)
    }

    pub fn payload(&self) -> &[u8] {
        if self.data.len() <= 1 {
            &[]
        } else {
            &self.data[1..]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nal_unit_parsing() {
        let data = vec![
            0x00, 0x00, 0x01, 0x67, 0x01, 0x02, // SPS
            0x00, 0x00, 0x01, 0x68, 0x03, 0x04, // PPS
            0x00, 0x00, 0x01, 0x65, 0x05, 0x06, // IDR
        ];

        let units = NALUnit::find_units(&data).unwrap();
        assert_eq!(units.len(), 3);

        assert_eq!(units[0].nal_type().unwrap(), NALType::SPS);
        assert_eq!(units[1].nal_type().unwrap(), NALType::PPS);
        assert_eq!(units[2].nal_type().unwrap(), NALType::IDR);

        assert_eq!(units[0].payload(), &[0x01, 0x02]);
        assert_eq!(units[1].payload(), &[0x03, 0x04]);
        assert_eq!(units[2].payload(), &[0x05, 0x06]);
    }
}
