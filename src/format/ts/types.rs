use crate::error::Result;
use bytes::{BufMut, BytesMut};
use std::time::Duration;

// Stream IDs
/// Stream ID for H.264 video streams in PES packets
pub const STREAM_ID_H264: u8 = 0xe0;
/// Stream ID for H.265 video streams in PES packets
pub const STREAM_ID_H265: u8 = 0xe1;
/// Stream ID for AAC audio streams in PES packets
pub const STREAM_ID_AAC: u8 = 0xc0;

// PIDs
/// PID for Program Association Table (PAT)
pub const PID_PAT: u16 = 0x0000;
/// PID for Program Map Table (PMT)
pub const PID_PMT: u16 = 0x1000;

// Table IDs
/// Table ID for Program Association Table (PAT)
pub const TABLE_ID_PAT: u8 = 0x00;
/// Table ID for Program Map Table (PMT)
pub const TABLE_ID_PMT: u8 = 0x02;
/// Table extension for Program Association Table (PAT)
pub const TABLE_EXT_PAT: u16 = 1;
/// Table extension for Program Map Table (PMT)
pub const TABLE_EXT_PMT: u16 = 1;

// Elementary Stream Types
/// Stream type for H.264 video streams
pub const STREAM_TYPE_H264: u8 = 0x1b;
/// Stream type for H.265 video streams
pub const STREAM_TYPE_H265: u8 = 0x24;
/// Stream type for AAC audio streams
pub const STREAM_TYPE_AAC: u8 = 0x0f;
/// Stream type for Alignment Descriptor
pub const STREAM_TYPE_ALIGNMENT_DESCRIPTOR: u8 = 0x06;

// Constants
/// Size of a Transport Stream packet in bytes
pub const TS_PACKET_SIZE: usize = 188;
/// Size of a Transport Stream header in bytes
pub const TS_HEADER_SIZE: usize = 4;
/// Maximum size of a PES header in bytes
pub const MAX_PES_HEADER_SIZE: usize = 19;
/// Clock frequency for Presentation Time Stamps (PTS) in Hz
pub const PTS_HZ: u64 = 90_000;
/// Clock frequency for Program Clock Reference (PCR) in Hz
pub const PCR_HZ: u64 = 27_000_000;

/// Represents an entry in the Program Association Table (PAT)
///
/// Each entry maps a program number to a PID (Program Identifier).
/// For program_number 0, it indicates the Network PID. For other program numbers,
/// it indicates the PID of the Program Map Table (PMT) for that program.
#[derive(Debug, Clone)]
pub struct PATEntry {
    /// Program number (16-bit)
    ///
    /// Identifies a program within the Transport Stream.
    /// Value 0 is reserved for Network PID.
    pub program_number: u16,
    /// Network PID (13-bit)
    ///
    /// PID for the Network Information Table (NIT) when program_number is 0.
    /// Otherwise, should be ignored.
    pub network_pid: u16,
    /// Program Map PID (13-bit)
    ///
    /// PID of the Program Map Table (PMT) associated with this program.
    /// Only valid when program_number is not 0.
    pub program_map_pid: u16,
}

/// Program Association Table (PAT) in MPEG Transport Stream
///
/// The PAT is a special table that maps program numbers to PMT PIDs.
/// It is always transmitted on PID 0x0000 and is essential for demultiplexing
/// the transport stream.
#[derive(Debug, Clone, Default)]
pub struct PAT {
    /// Vector of PAT entries, each mapping a program number to PMT PID
    pub entries: Vec<PATEntry>,
}

impl PAT {
    /// Creates a new empty PAT
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Returns the length in bytes needed to store the PAT entries
    ///
    /// Each PAT entry requires 4 bytes: 2 for program_number and 2 for PID.
    pub fn len(&self) -> usize {
        self.entries.len() * 4
    }

    /// Writes the PAT entries to a BytesMut buffer
    ///
    /// Each entry is written as a program_number (16 bits) followed by
    /// either network_PID (for program_number 0) or program_map_PID (for other numbers).
    ///
    /// # Arguments
    ///
    /// * `buf` - BytesMut buffer to write the PAT entries to
    ///
    /// # Returns
    ///
    /// `Ok(())` if writing was successful, `Err` otherwise
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

/// A descriptor providing additional information about programs or elementary streams
///
/// Descriptors are used in both PMT and elementary stream info to provide
/// supplementary information about the stream or program.
#[derive(Debug, Clone)]
pub struct Descriptor {
    /// Tag identifying the descriptor type
    pub tag: u8,
    /// Raw descriptor data bytes
    pub data: Vec<u8>,
}

/// Represents elementary stream information in PMT
///
/// Each PMT contains one or more elementary stream info entries,
/// describing the type and PID of each elementary stream (audio, video, etc.)
#[derive(Debug, Clone)]
pub struct ElementaryStreamInfo {
    /// Elementary stream type (8-bit)
    ///
    /// Indicates the encoding format of the elementary stream (e.g., H.264, AAC).
    pub stream_type: u8,
    /// Elementary PID (13-bit)
    ///
    /// PID of the packets carrying the elementary stream data.
    pub elementary_pid: u16,
    /// Descriptors for the elementary stream (variable length)
    ///
    /// Provides additional information about the elementary stream format and properties.
    pub descriptors: Vec<Descriptor>,
}

/// Program Map Table (PMT) for a program in MPEG Transport Stream
///
/// The PMT provides the mappings between program numbers and the PIDs of their
/// elementary streams (video, audio, etc.). Each program has its own PMT.
#[derive(Debug, Clone, Default)]
pub struct PMT {
    /// PID carrying the Program Clock Reference (PCR)
    pub pcr_pid: u16,
    /// Descriptors that apply to the whole program
    pub program_descriptors: Vec<Descriptor>,
    /// Information about each elementary stream in the program
    pub elementary_stream_infos: Vec<ElementaryStreamInfo>,
}

impl PMT {
    /// Creates a new empty Program Map Table
    ///
    /// Initializes a PMT with default values (PCR PID 0) and empty descriptor
    /// and elementary stream info lists.
    pub fn new() -> Self {
        Self {
            pcr_pid: 0,
            program_descriptors: Vec::new(),
            elementary_stream_infos: Vec::new(),
        }
    }

    /// Returns the length in bytes of the PMT section
    ///
    /// Calculates total size including PCR PID, program info length,
    /// program descriptors, and all elementary stream information.
    ///
    /// # Returns
    ///
    /// Total length of PMT section in bytes
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

    /// Writes the PMT section to a BytesMut buffer
    ///
    /// This method writes the PMT header, program descriptors, and elementary
    /// stream information loops to the buffer according to MPEG-TS format.
    ///
    /// # Arguments
    ///
    /// * `buf` - BytesMut buffer to write the PMT section to
    ///
    /// # Returns
    ///
    /// `Ok(())` if writing was successful, `Err` otherwise
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

/// Represents an Adaptation Field in MPEG Transport Stream packets
///
/// The adaptation field is an optional part of a TS packet header that provides
/// control information, timing information (PCR), and padding.
#[derive(Debug, Clone)]
pub struct AdaptationField {
    /// Adaptation field length (8-bit)
    ///
    /// Indicates the number of bytes in the adaptation field following this byte.
    pub length: usize,
    /// Discontinuity indicator (1-bit)
    ///
    /// Set to 1 if there is a discontinuity in the stream (e.g., codec reset).
    pub discontinuity: bool,
    /// Random access indicator (1-bit)
    ///
    /// Set to 1 if the start of a stream or a key frame is present.
    pub random_access: bool,
    /// Elementary stream priority indicator (1-bit)
    ///
    /// Indicates if the ES is priority stream (e.g., for error concealment).
    pub es_priority: bool,
    /// PCR flag (1-bit)
    ///
    /// Set to 1 if a Program Clock Reference (PCR) is present in the adaptation field.
    pub pcr_flag: bool,
    /// OPCR flag (1-bit)
    ///
    /// Set to 1 if an Original Program Clock Reference (OPCR) is present.
    pub opcr_flag: bool,
    /// Splicing point flag (1-bit)
    ///
    /// Set to 1 if a splicing point is present, indicating stream splicing.
    pub splicing_point_flag: bool,
    /// Private data flag (1-bit)
    ///
    /// Set to 1 if private data bytes are present in the adaptation field.
    pub private_data_flag: bool,
    /// Adaptation field extension flag (1-bit)
    ///
    /// Set to 1 if an adaptation field extension is present.
    pub extension_flag: bool,
    /// Program Clock Reference (PCR) value (42-bit)
    ///
    /// Optional PCR value used for synchronization and timing recovery.
    pub pcr: Option<u64>,
    /// Original Program Clock Reference (OPCR) value (42-bit)
    ///
    /// Optional OPCR value for original program PCR when transcoding.
    pub opcr: Option<u64>,
    /// Splice countdown (8-bit signed)
    ///
    /// Indicates the number of TS packets until a splicing point.
    pub splice_countdown: Option<i8>,
    /// Private data bytes (variable length)
    ///
    /// Optional private data bytes, if private_data_flag is set.
    pub private_data: Option<Vec<u8>>,
}

/// Represents a Transport Stream (TS) packet header
///
/// The TS header is the fixed 4-byte prefix of each TS packet, providing
/// essential packet identification and control information.
#[derive(Debug)]
pub struct TSHeader {
    /// Sync byte (8-bit), always 0x47 to identify start of a TS packet
    pub sync_byte: u8, // Always 0x47
    /// Transport error indicator (1-bit)
    ///
    /// Set to 1 if there is an uncorrectable error in the packet.
    pub transport_error: bool,
    /// Payload unit start indicator (1-bit)
    ///
    /// Set to 1 if the PES packet or PSI section starts at the beginning of the payload.
    pub payload_unit_start: bool,
    /// Transport priority (1-bit)
    ///
    /// Set to 1 if the current packet has a higher priority than other packets with the same PID.
    pub transport_priority: bool,
    /// PID (13-bit)
    ///
    /// Packet Identifier, used to demultiplex packets of different elementary streams and PSI tables.
    pub pid: u16,
    /// Transport scrambling control (2-bit)
    ///
    /// Indicates the scrambling mode of the payload (e.g., not scrambled, scrambled with even/odd key).
    pub scrambling_control: u8,
    /// Adaptation field control (2-bit)
    ///
    /// Indicates if an adaptation field and/or payload is present in the TS packet.
    pub adaptation_field_exists: bool,
    /// Payload presence indicator (for clarity, not explicitly in header bits)
    pub contains_payload: bool,
    /// Continuity counter (4-bit)
    ///
    /// Counter incrementing modulo 16 for each TS packet with the same PID,
    /// used for detecting packet loss and reassembling PES packets.
    pub continuity_counter: u8,
}

impl Default for TSHeader {
    /// Creates a default TSHeader with standard values
    ///
    /// Default header has sync byte 0x47 and no flags set, suitable for
    /// starting point or when specific header values are not required.
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
    /// Writes the TS header to a BytesMut buffer
    ///
    /// Packs the TS header fields into a 4-byte array and writes it to the buffer.
    /// Fields are placed in bit-correct positions according to MPEG-TS specification.
    ///
    /// # Arguments
    ///
    /// * `buf` - BytesMut buffer to write the header to
    ///
    /// # Returns
    ///
    /// `Ok(())` if writing was successful, `Err` otherwise
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

/// Converts a Program Clock Reference (PCR) value to a Duration
///
/// PCR is a 42-bit value used for timing synchronization in MPEG-TS.
/// It consists of a 33-bit base and a 9-bit extension.
///
/// # Arguments
///
/// * `pcr` - 42-bit PCR value to convert
///
/// # Returns
///
/// Duration representing the time value of the PCR
pub fn pcr_to_time(pcr: u64) -> Duration {
    let base = pcr >> 15;
    let ext = pcr & 0x1ff;
    let ts = base * 300 + ext;
    Duration::from_nanos((ts * 1_000_000_000) / PCR_HZ)
}

/// Converts a Duration to a Program Clock Reference (PCR) value
///
/// Creates a 42-bit PCR value from a Duration, maintaining the
/// required precision for MPEG-TS timing synchronization.
///
/// # Arguments
///
/// * `time` - Duration to convert to PCR
///
/// # Returns
///
/// 42-bit PCR value
pub fn time_to_pcr(time: Duration) -> u64 {
    let ts = time.as_nanos() as u64 * PCR_HZ / 1_000_000_000;
    let base = ts / 300;
    let ext = ts % 300;
    base << 15 | 0x3f << 9 | ext
}

/// Converts a Presentation Time Stamp (PTS) value to a Duration
///
/// PTS uses a 90kHz clock for timing presentation of audio and video frames.
///
/// # Arguments
///
/// * `pts` - PTS value to convert
///
/// # Returns
///
/// Duration representing the presentation time
pub fn pts_to_time(pts: u64) -> Duration {
    Duration::from_nanos((pts * 1_000_000_000) / PTS_HZ)
}

/// Converts a Duration to a Presentation Time Stamp (PTS) value
///
/// Creates a PTS value suitable for MPEG-TS audio/video timing,
/// using the 90kHz PTS clock frequency.
///
/// # Arguments
///
/// * `time` - Duration to convert to PTS
///
/// # Returns
///
/// PTS value at 90kHz clock rate
pub fn time_to_pts(time: Duration) -> u64 {
    time.as_nanos() as u64 * PTS_HZ / 1_000_000_000
}
