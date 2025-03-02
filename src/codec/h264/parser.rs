use crate::error::{Result, VdkError};

/// H.264/AVC Network Abstraction Layer (NAL) unit types as defined in ITU-T H.264
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NALType {
    /// Unspecified NAL unit type
    Unknown = 0,
    /// Coded slice of a non-IDR picture
    NonIDR = 1,
    /// Coded slice of an IDR picture (key frame)
    IDR = 5,
    /// Supplemental Enhancement Information
    SEI = 6,
    /// Sequence Parameter Set
    SPS = 7,
    /// Picture Parameter Set
    PPS = 8,
}

impl NALType {
    /// Creates a NALType from the 5-bit type field in the NAL unit header
    ///
    /// # Arguments
    ///
    /// * `val` - NAL unit header byte
    ///
    /// # Returns
    ///
    /// * `Ok(NALType)` - The parsed NAL unit type
    /// * `Err(_)` - If the type value is not recognized
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

/// An H.264/AVC Network Abstraction Layer (NAL) unit
///
/// Represents a single NAL unit found in an H.264 bitstream, containing header and payload data.
/// NAL units are the basic containers for both parameter sets and coded slice data.
#[derive(Debug)]
pub struct NALUnit<'a> {
    /// Raw NAL unit data including header byte and payload
    pub data: &'a [u8],
}

impl<'a> NALUnit<'a> {
    /// Finds all NAL units in a byte slice by locating start codes (0x000001 or 0x00000001)
    ///
    /// Scans through the input data looking for NAL unit start codes and extracts each unit.
    /// Handles both 3-byte (0x000001) and 4-byte (0x00000001) start codes.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice containing H.264 bitstream data
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<NALUnit>)` - Vector of found NAL units
    /// * `Err(_)` - If parsing fails
    pub fn find_units(data: &'a [u8]) -> Result<Vec<NALUnit<'a>>> {
        let mut units = Vec::new();
        let mut start = 0;

        // Find start codes
        for i in 0..data.len() - 3 {
            if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
                if start > 0 && start < i {
                    units.push(NALUnit {
                        data: &data[start..i],
                    });
                }
                start = i + 3;
            }
            if i > 0 && data[i - 1] == 0 && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
                if start > 0 && start < i - 1 {
                    units.push(NALUnit {
                        data: &data[start..(i - 1)],
                    });
                }
                start = i + 3;
            }
        }

        // Add final unit
        if start > 0 && start < data.len() {
            units.push(NALUnit {
                data: &data[start..],
            });
        }

        Ok(units)
    }

    /// Returns the NAL unit header byte
    ///
    /// The header byte contains the NAL unit type and other flags.
    /// The first bit is the forbidden_zero_bit, next two bits are nal_ref_idc,
    /// and the remaining 5 bits are the nal_unit_type.
    ///
    /// # Returns
    ///
    /// * `Ok(u8)` - The header byte
    /// * `Err(_)` - If the NAL unit is empty
    pub fn header(&self) -> Result<u8> {
        self.data
            .first()
            .copied()
            .ok_or_else(|| VdkError::Codec("Empty NAL unit".into()))
    }

    /// Returns the type of this NAL unit
    ///
    /// Parses the NAL unit type from the 5 least significant bits of the header byte.
    /// This determines the content type (e.g., IDR frame, parameter set, etc.).
    ///
    /// # Returns
    ///
    /// * `Ok(NALType)` - The parsed NAL unit type
    /// * `Err(_)` - If header parsing fails or type is unrecognized
    pub fn nal_type(&self) -> Result<NALType> {
        let header = self.header()?;
        NALType::from_u8(header)
    }

    /// Returns the NAL unit payload (data after header byte)
    ///
    /// The payload contains the actual data of the NAL unit, excluding the header byte.
    /// For parameter sets (SPS/PPS) this contains configuration data.
    /// For coded slices this contains the compressed video data.
    ///
    /// # Returns
    ///
    /// * Empty slice if NAL unit has no payload or is invalid
    /// * Slice containing data after header byte otherwise
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
