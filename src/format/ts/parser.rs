use super::types::*;
use crate::error::{Result, VdkError};
use bytes::BytesMut;

#[allow(dead_code)]
pub struct TSPacketParser {
    buffer: BytesMut,
}

impl TSPacketParser {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
        }
    }

    pub fn parse_header(&self, data: &[u8]) -> Result<TSHeader> {
        if data.len() < TS_HEADER_SIZE {
            return Err(VdkError::InvalidData("TS packet too short".into()));
        }

        if data[0] != 0x47 {
            return Err(VdkError::InvalidData("Invalid sync byte".into()));
        }

        Ok(TSHeader {
            sync_byte: data[0],
            transport_error: (data[1] & 0x80) != 0,
            payload_unit_start: (data[1] & 0x40) != 0,
            transport_priority: (data[1] & 0x20) != 0,
            pid: (((data[1] & 0x1F) as u16) << 8) | data[2] as u16,
            scrambling_control: (data[3] >> 6) & 0x03,
            adaptation_field_exists: (data[3] & 0x20) != 0,
            contains_payload: (data[3] & 0x10) != 0,
            continuity_counter: data[3] & 0x0F,
        })
    }

    pub fn parse_adaptation_field(
        &self,
        data: &[u8],
        offset: usize,
    ) -> Result<Option<AdaptationField>> {
        if (data[3] & 0x20) == 0 {
            return Ok(None);
        }

        let adaptation_field_length = data[offset] as usize;
        if adaptation_field_length == 0 {
            return Ok(None);
        }

        if data.len() < offset + adaptation_field_length + 1 {
            return Err(VdkError::InvalidData("Adaptation field too short".into()));
        }

        let flags = data[offset + 1];
        let mut field = AdaptationField {
            length: adaptation_field_length,
            discontinuity: (flags & 0x80) != 0,
            random_access: (flags & 0x40) != 0,
            es_priority: (flags & 0x20) != 0,
            pcr_flag: (flags & 0x10) != 0,
            opcr_flag: (flags & 0x08) != 0,
            splicing_point_flag: (flags & 0x04) != 0,
            private_data_flag: (flags & 0x02) != 0,
            extension_flag: (flags & 0x01) != 0,
            pcr: None,
            opcr: None,
            splice_countdown: None,
            private_data: None,
        };

        let mut pos = offset + 2;

        if field.pcr_flag {
            if data.len() < pos + 6 {
                return Err(VdkError::InvalidData("PCR data too short".into()));
            }
            let pcr_base = ((data[pos] as u64) << 25)
                | ((data[pos + 1] as u64) << 17)
                | ((data[pos + 2] as u64) << 9)
                | ((data[pos + 3] as u64) << 1)
                | ((data[pos + 4] & 0x80) as u64 >> 7);
            let pcr_ext = (((data[pos + 4] & 0x01) as u64) << 8) | (data[pos + 5] as u64);
            field.pcr = Some(pcr_base * 300 + pcr_ext);
            pos += 6;
        }

        if field.opcr_flag {
            if data.len() < pos + 6 {
                return Err(VdkError::InvalidData("OPCR data too short".into()));
            }
            let opcr_base = ((data[pos] as u64) << 25)
                | ((data[pos + 1] as u64) << 17)
                | ((data[pos + 2] as u64) << 9)
                | ((data[pos + 3] as u64) << 1)
                | ((data[pos + 4] & 0x80) as u64 >> 7);
            let opcr_ext = (((data[pos + 4] & 0x01) as u64) << 8) | (data[pos + 5] as u64);
            field.opcr = Some(opcr_base * 300 + opcr_ext);
            pos += 6;
        }

        if field.splicing_point_flag {
            if data.len() < pos + 1 {
                return Err(VdkError::InvalidData("Splice countdown too short".into()));
            }
            field.splice_countdown = Some(data[pos] as i8);
            pos += 1;
        }

        if field.private_data_flag {
            if data.len() < pos + 1 {
                return Err(VdkError::InvalidData(
                    "Private data length byte missing".into(),
                ));
            }
            let private_data_length = data[pos] as usize;
            pos += 1;
            if data.len() < pos + private_data_length {
                return Err(VdkError::InvalidData("Private data too short".into()));
            }
            field.private_data = Some(data[pos..pos + private_data_length].to_vec());
        }
        Ok(Some(field))
    }

    pub fn parse_pat(&self, data: &[u8], offset: usize, length: usize) -> Result<PAT> {
        let mut pat = PAT::new();
        let mut pos = offset;
        let end = offset + length;

        while pos + 4 <= end {
            let program_number = ((data[pos] as u16) << 8) | data[pos + 1] as u16;
            let pid = ((data[pos + 2] as u16) << 8) | data[pos + 3] as u16;
            pat.entries.push(PATEntry {
                program_number,
                network_pid: if program_number == 0 { pid } else { 0 },
                program_map_pid: if program_number != 0 { pid } else { 0 },
            });
            pos += 4;
        }

        Ok(pat)
    }

    pub fn parse_pmt(&self, data: &[u8], offset: usize, length: usize) -> Result<PMT> {
        let mut pmt = PMT::new();
        let mut pos = offset;
        let end = offset + length;

        if pos + 2 > end {
            return Err(VdkError::InvalidData("PMT too short for PCR PID".into()));
        }

        pmt.pcr_pid = ((data[pos] as u16 & 0x1F) << 8) | data[pos + 1] as u16;
        pos += 2;

        if pos + 2 > end {
            return Err(VdkError::InvalidData(
                "PMT too short for program info length".into(),
            ));
        }

        let program_info_length = ((data[pos] as usize & 0x0F) << 8) | data[pos + 1] as usize;
        pos += 2;

        if program_info_length > 0 {
            if pos + program_info_length > end {
                return Err(VdkError::InvalidData("Program info data too short".into()));
            }
            pmt.program_descriptors =
                self.parse_descriptors(&data[pos..pos + program_info_length])?;
            pos += program_info_length;
        }

        while pos + 5 <= end {
            let stream_type = data[pos];
            let elementary_pid = ((data[pos + 1] as u16 & 0x1F) << 8) | data[pos + 2] as u16;
            let es_info_length = ((data[pos + 3] as usize & 0x0F) << 8) | data[pos + 4] as usize;
            pos += 5;

            if pos + es_info_length > end {
                return Err(VdkError::InvalidData("ES info data too short".into()));
            }

            let descriptors = self.parse_descriptors(&data[pos..pos + es_info_length])?;
            pos += es_info_length;

            pmt.elementary_stream_infos.push(ElementaryStreamInfo {
                stream_type,
                elementary_pid,
                descriptors,
            });
        }

        Ok(pmt)
    }

    fn parse_descriptors(&self, data: &[u8]) -> Result<Vec<Descriptor>> {
        let mut descriptors = Vec::new();
        let mut pos = 0;

        while pos + 2 <= data.len() {
            let tag = data[pos];
            let length = data[pos + 1] as usize;
            pos += 2;

            if pos + length > data.len() {
                return Err(VdkError::InvalidData("Descriptor data too short".into()));
            }

            descriptors.push(Descriptor {
                tag,
                data: data[pos..pos + length].to_vec(),
            });
            pos += length;
        }

        Ok(descriptors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ts_header() {
        let parser = TSPacketParser::new();
        let data = [
            0x47, // Sync byte
            0x40, // Payload unit start indicator set
            0x00, // PID (high bits)
            0x10, // Continuity counter
        ];

        let header = parser.parse_header(&data).unwrap();
        assert_eq!(header.sync_byte, 0x47);
        assert!(header.payload_unit_start);
        assert_eq!(header.pid, 0);
        assert_eq!(header.continuity_counter, 0x10 & 0x0F);
    }

    #[test]
    fn test_parse_pat() {
        let parser = TSPacketParser::new();
        let data = [
            0x00, 0x01, // Program number
            0x10, 0x00, // PID
            0x00, 0x02, // Program number
            0x20, 0x00, // PID
        ];

        let pat = parser.parse_pat(&data, 0, data.len()).unwrap();
        assert_eq!(pat.entries.len(), 2);
        assert_eq!(pat.entries[0].program_number, 1);
        assert_eq!(pat.entries[0].program_map_pid, 0x1000);
        assert_eq!(pat.entries[1].program_number, 2);
        assert_eq!(pat.entries[1].program_map_pid, 0x2000);
    }
}
