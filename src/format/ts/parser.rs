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
            if pos + 1 > adaptation_field_length + offset {
                return Err(VdkError::InvalidData(
                    "Private data length byte missing".into(),
                ));
            }
            let private_data_length = data[pos] as usize;
            pos += 1;
            let remaining = adaptation_field_length - (pos - offset);
            if private_data_length > remaining {
                // Invalid private data length but try to continue parsing
                field.private_data = None;
                return Ok(Some(field));
            }
            field.private_data = Some(data[pos..pos + private_data_length].to_vec());
        }

        Ok(Some(field))
    }

    pub fn parse_pat(&self, data: &[u8], _offset: usize, _length: usize) -> Result<PAT> {
        println!("Parsing PAT: len={}", data.len());
        let mut pat = PAT::new();
        
        if data.len() < 8 {
            return Err(VdkError::InvalidData("PAT section too short".into()));
        }

        if data[0] != TABLE_ID_PAT {
            return Err(VdkError::InvalidData(format!("Invalid PAT table ID: 0x{:02x}", data[0])));
        }

        let section_length = ((data[1] as usize & 0x0F) << 8) | data[2] as usize;
        let total_length = 3 + section_length;
        println!("  PAT section length: {}", section_length);

        if data.len() < total_length {
            return Err(VdkError::InvalidData("PAT data shorter than section length".into()));
        }

        // Skip to start of programs (past TSID, version, section numbers)
        let mut pos = 8;
        
        // Read program entries until CRC
        while pos + 4 <= total_length - 4 {
            let program_number = ((data[pos] as u16) << 8) | data[pos + 1] as u16;
            let pid = ((data[pos + 2] as u16 & 0x1F) << 8) | data[pos + 3] as u16;
            
            println!("  Program: number={}, pid=0x{:04x}", program_number, pid);
            
            pat.entries.push(PATEntry {
                program_number,
                network_pid: if program_number == 0 { pid } else { 0 },
                program_map_pid: if program_number != 0 { pid } else { 0 },
            });
            pos += 4;
        }

        Ok(pat)
    }

    pub fn parse_pmt(&self, data: &[u8], _offset: usize, _length: usize) -> Result<PMT> {
        println!("Parsing PMT: len={}", data.len());
        let mut pmt = PMT::new();

        if data.len() < 7 {
            return Err(VdkError::InvalidData("PMT section too short".into()));
        }

        // Verify table ID
        if data[0] != TABLE_ID_PMT {
            return Err(VdkError::InvalidData(format!("Invalid PMT table ID: 0x{:02x}", data[0])));
        }

        // Get section length
        let section_length = ((data[1] as usize & 0x0F) << 8) | data[2] as usize;
        let total_length = 3 + section_length;
        println!("  PMT section length: {}", section_length);

        if data.len() < total_length {
            return Err(VdkError::InvalidData("PMT data shorter than section length".into()));
        }

        // Skip transport stream ID (2 bytes)
        // Skip version and current_next indicator (1 byte)
        // Skip section number and last section number (2 bytes)
        let mut pos = 8;

        // Parse PCR PID
        pmt.pcr_pid = ((data[pos] as u16 & 0x1F) << 8) | data[pos + 1] as u16;
        println!("  PCR PID: 0x{:04x}", pmt.pcr_pid);
        pos += 2;

        // Parse program info
        let program_info_length = ((data[pos] as usize & 0x0F) << 8) | data[pos + 1] as usize;
        println!("  Program info length: {}", program_info_length);
        pos += 2;

        if program_info_length > 0 {
            if pos + program_info_length > total_length - 4 {
                return Err(VdkError::InvalidData("Program info extends beyond section".into()));
            }
            pmt.program_descriptors = self.parse_descriptors(&data[pos..pos + program_info_length])?;
            println!("  Parsed {} program descriptors", pmt.program_descriptors.len());
            pos += program_info_length;
        }

        // Parse elementary streams until CRC
        while pos + 5 <= total_length - 4 {
            let stream_type = data[pos];
            let elementary_pid = ((data[pos + 1] as u16 & 0x1F) << 8) | data[pos + 2] as u16;
            let es_info_length = ((data[pos + 3] as usize & 0x0F) << 8) | data[pos + 4] as usize;
            pos += 5;

            println!("  Stream: type=0x{:02x}, pid=0x{:04x}, info_len={}", 
                    stream_type, elementary_pid, es_info_length);

            if pos + es_info_length > total_length - 4 {
                return Err(VdkError::InvalidData("ES info extends beyond section".into()));
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
            TABLE_ID_PAT,
            0x80, 0x0D, // Section length (13 bytes)
            0x00, 0x01, // Transport stream ID
            0xC1, // Version and current_next
            0x00, 0x00, // Section numbers
            0x00, 0x01, // Program number
            0x10, 0x00, // PMT PID
            0x00, 0x00, 0x00, 0x00, // CRC32
        ];

        let pat = parser.parse_pat(&data, 0, data.len()).unwrap();
        assert_eq!(pat.entries.len(), 1);
        assert_eq!(pat.entries[0].program_number, 1);
        assert_eq!(pat.entries[0].program_map_pid, 0x1000);
    }
}
