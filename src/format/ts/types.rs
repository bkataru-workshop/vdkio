use crate::error::Result;
use bytes::{BufMut, BytesMut};
use std::time::Duration;

// Stream IDs
pub const STREAM_ID_H264: u8 = 0xe0;
pub const STREAM_ID_H265: u8 = 0xe1;
pub const STREAM_ID_AAC: u8 = 0xc0;

// PIDs
pub const PID_PAT: u16 = 0x0000;
pub const PID_PMT: u16 = 0x1000;

// Table IDs
pub const TABLE_ID_PAT: u8 = 0x00;
pub const TABLE_ID_PMT: u8 = 0x02;
pub const TABLE_EXT_PAT: u16 = 1;
pub const TABLE_EXT_PMT: u16 = 1;

// Elementary Stream Types
pub const STREAM_TYPE_H264: u8 = 0x1b;
pub const STREAM_TYPE_H265: u8 = 0x24;
pub const STREAM_TYPE_AAC: u8 = 0x0f;
pub const STREAM_TYPE_ALIGNMENT_DESCRIPTOR: u8 = 0x06;

// Constants
pub const TS_PACKET_SIZE: usize = 188;
pub const TS_HEADER_SIZE: usize = 4;
pub const MAX_PES_HEADER_SIZE: usize = 19;
pub const PTS_HZ: u64 = 90_000;
pub const PCR_HZ: u64 = 27_000_000;

#[derive(Debug, Clone)]
pub struct PATEntry {
    pub program_number: u16,
    pub network_pid: u16,
    pub program_map_pid: u16,
}

#[derive(Debug, Clone, Default)]
pub struct PAT {
    pub entries: Vec<PATEntry>,
}

impl PAT {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len() * 4
    }

    pub fn write_to(&self, buf: &mut BytesMut) -> Result<()> {
        for entry in &self.entries {
            buf.put_u16(entry.program_number);
            if entry.program_number == 0 {
                buf.put_u16(entry.network_pid & 0x1fff | 7 << 13);
            } else {
                buf.put_u16(entry.program_map_pid & 0x1fff | 7 << 13);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Descriptor {
    pub tag: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ElementaryStreamInfo {
    pub stream_type: u8,
    pub elementary_pid: u16,
    pub descriptors: Vec<Descriptor>,
}

#[derive(Debug, Clone, Default)]
pub struct PMT {
    pub pcr_pid: u16,
    pub program_descriptors: Vec<Descriptor>,
    pub elementary_stream_infos: Vec<ElementaryStreamInfo>,
}

impl PMT {
    pub fn new() -> Self {
        Self {
            pcr_pid: 0,
            program_descriptors: Vec::new(),
            elementary_stream_infos: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        let mut n = 4; // PCRPID + program info length

        // Program descriptors
        for desc in &self.program_descriptors {
            n += 2 + desc.data.len();
        }

        // Elementary stream infos
        for info in &self.elementary_stream_infos {
            n += 5; // stream_type + elementary_pid + ES info length
            for desc in &info.descriptors {
                n += 2 + desc.data.len();
            }
        }

        n
    }

    pub fn write_to(&self, buf: &mut BytesMut) -> Result<()> {
        // Write PCR PID
        buf.put_u16(self.pcr_pid & 0x1fff | 7 << 13);

        // Program descriptors
        let prog_desc_len = self
            .program_descriptors
            .iter()
            .map(|d| 2 + d.data.len())
            .sum::<usize>();
        buf.put_u16((prog_desc_len as u16) & 0x3ff | 0xf << 12);

        for desc in &self.program_descriptors {
            buf.put_u8(desc.tag);
            buf.put_u8(desc.data.len() as u8);
            buf.put_slice(&desc.data);
        }

        // Elementary stream infos
        for info in &self.elementary_stream_infos {
            buf.put_u8(info.stream_type);
            buf.put_u16(info.elementary_pid & 0x1fff | 7 << 13);

            let es_desc_len = info
                .descriptors
                .iter()
                .map(|d| 2 + d.data.len())
                .sum::<usize>();
            buf.put_u16((es_desc_len as u16) & 0x3ff | 0xf << 12);

            for desc in &info.descriptors {
                buf.put_u8(desc.tag);
                buf.put_u8(desc.data.len() as u8);
                buf.put_slice(&desc.data);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AdaptationField {
    pub length: usize,
    pub discontinuity: bool,
    pub random_access: bool,
    pub es_priority: bool,
    pub pcr_flag: bool,
    pub opcr_flag: bool,
    pub splicing_point_flag: bool,
    pub private_data_flag: bool,
    pub extension_flag: bool,
    pub pcr: Option<u64>,
    pub opcr: Option<u64>,
    pub splice_countdown: Option<i8>,
    pub private_data: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct TSHeader {
    pub sync_byte: u8, // Always 0x47
    pub transport_error: bool,
    pub payload_unit_start: bool,
    pub transport_priority: bool,
    pub pid: u16,
    pub scrambling_control: u8,
    pub adaptation_field_exists: bool,
    pub contains_payload: bool,
    pub continuity_counter: u8,
}

impl Default for TSHeader {
    fn default() -> Self {
        Self {
            sync_byte: 0x47,
            transport_error: false,
            payload_unit_start: false,
            transport_priority: false,
            pid: 0,
            scrambling_control: 0,
            adaptation_field_exists: false,
            contains_payload: true,
            continuity_counter: 0,
        }
    }
}

impl TSHeader {
    pub fn write_to(&self, buf: &mut BytesMut) -> Result<()> {
        buf.put_u8(self.sync_byte);

        let mut b1 = 0u8;
        if self.transport_error {
            b1 |= 0x80;
        }
        if self.payload_unit_start {
            b1 |= 0x40;
        }
        if self.transport_priority {
            b1 |= 0x20;
        }
        b1 |= ((self.pid >> 8) & 0x1f) as u8;
        buf.put_u8(b1);

        buf.put_u8((self.pid & 0xff) as u8);

        let mut b3 = self.scrambling_control << 6;
        if self.adaptation_field_exists {
            b3 |= 0x20;
        }
        if self.contains_payload {
            b3 |= 0x10;
        }
        b3 |= self.continuity_counter & 0x0f;
        buf.put_u8(b3);

        Ok(())
    }
}

// Time conversion utilities
pub fn pcr_to_time(pcr: u64) -> Duration {
    let base = pcr >> 15;
    let ext = pcr & 0x1ff;
    let ts = base * 300 + ext;
    Duration::from_nanos((ts * 1_000_000_000) / PCR_HZ)
}

pub fn time_to_pcr(time: Duration) -> u64 {
    let ts = time.as_nanos() as u64 * PCR_HZ / 1_000_000_000;
    let base = ts / 300;
    let ext = ts % 300;
    base << 15 | 0x3f << 9 | ext
}

pub fn pts_to_time(pts: u64) -> Duration {
    Duration::from_nanos((pts * 1_000_000_000) / PTS_HZ)
}

pub fn time_to_pts(time: Duration) -> u64 {
    time.as_nanos() as u64 * PTS_HZ / 1_000_000_000
}
