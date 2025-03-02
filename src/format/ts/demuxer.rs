use super::parser::TSPacketParser;
use super::types::*;
use crate::av::{self, CodecData, CodecType, Packet};
use crate::error::Result;
use crate::format::Demuxer as FormatDemuxer;
use crate::utils::crc::Crc32Mpeg2;
use bytes::Bytes;
use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncReadExt};

/// MPEG Transport Stream demuxer.
///
/// This demuxer extracts elementary streams from MPEG-TS container format.
/// It supports:
/// - PAT/PMT parsing for stream information
/// - PES packet extraction and reassembly
/// - PCR timing recovery
pub struct TSDemuxer<R: AsyncRead + Unpin + Send> {
    reader: R,
    parser: TSPacketParser,
    streams: HashMap<u16, StreamInfo>,
    pmt_pid: Option<u16>,
    pmt: Option<PMT>,
}

/// Information about individual elementary streams.
#[derive(Debug)]
struct StreamInfo {
    /// Index in the stream list
    stream_index: usize,
    /// Stream configuration data (codec info, etc.)
    config: Option<StreamCodecData>,
    /// Incomplete PES packet being assembled
    pes_buffer: Option<PESBuilder>,
}

/// Codec information for a stream.
#[derive(Debug, Clone)]
struct StreamCodecData {
    /// Type of codec (H264, AAC, etc.)
    codec_type: CodecType,
    /// Width in pixels for video codecs
    width: Option<u32>,
    /// Height in pixels for video codecs
    height: Option<u32>,
    /// Codec-specific extra data
    extra_data: Option<Vec<u8>>,
}

impl CodecData for StreamCodecData {
    fn codec_type(&self) -> CodecType {
        self.codec_type
    }
    fn width(&self) -> Option<u32> {
        self.width
    }
    fn height(&self) -> Option<u32> {
        self.height
    }
    fn extra_data(&self) -> Option<&[u8]> {
        self.extra_data.as_deref()
    }
}

// StreamCodecData implements Clone and CodecData, so it gets CodecDataExt through
// the blanket implementation in av::mod.rs

/// Helper for assembling PES packets from TS packets.
#[derive(Debug)]
struct PESBuilder {
    /// PTS from PES header
    pts: Option<i64>,
    /// Size of complete PES packet
    size: Option<usize>,
    /// Accumulated data
    data: Vec<u8>,
}

impl PESBuilder {
    fn new() -> Self {
        Self {
            pts: None,
            size: None,
            data: Vec::new(),
        }
    }

    /// Adds payload data to the packet being built.
    fn push_data(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    /// Sets PTS value from PES header.
    fn set_pts(&mut self, pts: i64) {
        self.pts = Some(pts);
    }

    /// Sets expected packet size from PES header.
    fn set_size(&mut self, size: usize) {
        self.size = Some(size);
    }

    /// Returns whether packet is complete.
    fn is_complete(&self) -> bool {
        if let Some(size) = self.size {
            self.data.len() >= size
        } else {
            false
        }
    }

    /// Takes the completed packet data.
    fn take_data(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.data)
    }
}

/// Parse PTS value from PES header.
fn parse_pts(data: &[u8]) -> Option<i64> {
    if data.len() < 14 || (data[7] & 0x80) == 0 {
        return None;
    }

    let pts = ((data[9] as i64 & 0x0E) << 29)
        | ((data[10] as i64) << 22)
        | ((data[11] as i64 & 0xFE) << 14)
        | ((data[12] as i64) << 7)
        | ((data[13] as i64 & 0xFE) >> 1);

    Some(pts)
}

impl<R: AsyncRead + Unpin + Send> TSDemuxer<R> {
    /// Creates a new TS demuxer.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            parser: TSPacketParser::new(),
            streams: HashMap::new(),
            pmt_pid: None,
            pmt: None,
        }
    }

    /// Reads a complete TS packet.
    async fn read_packet_data(&mut self) -> Result<Vec<u8>> {
        let mut packet = vec![0u8; TS_PACKET_SIZE];
        self.reader.read_exact(&mut packet).await?;
        Ok(packet)
    }
}

#[async_trait::async_trait]
impl<R: AsyncRead + Unpin + Send> FormatDemuxer for TSDemuxer<R> {
    async fn read_packet(&mut self) -> Result<Packet> {
        loop {
            let data = self.read_packet_data().await?;
            let header = self.parser.parse_header(&data)?;

            // Skip packets with transport errors
            if header.transport_error {
                continue;
            }

            // Parse adaptation field if present
            let mut payload_offset = TS_HEADER_SIZE;
            if header.adaptation_field_exists {
                if let Some(adaptation) = self.parser.parse_adaptation_field(&data, payload_offset)? {
                    payload_offset += adaptation.length + 1;
                }
            }

            // Handle payload
            if header.contains_payload {
                match header.pid {
                    PID_PAT if header.payload_unit_start => {
                        // Parse PAT to get PMT PID
                        let table_offset = payload_offset + data[payload_offset] as usize + 1;
                        let pat = self.parser.parse_pat(&data[table_offset..], 0, 0)?;
                        
                        // Use first program's PMT
                        if let Some(entry) = pat.entries.first() {
                            self.pmt_pid = Some(entry.program_map_pid);
                        }
                    }

                    pmt_pid if Some(pmt_pid) == self.pmt_pid && header.payload_unit_start => {
                        // Parse PMT to get elementary stream info
                        let table_offset = payload_offset + data[payload_offset] as usize + 1;
                        let pmt = self.parser.parse_pmt(&data[table_offset..], 0, 0)?;
                        
                        // Create stream entries
                        for (i, info) in pmt.elementary_stream_infos.iter().enumerate() {
                            let codec_type = match info.stream_type {
                                STREAM_TYPE_H264 => CodecType::H264,
                                STREAM_TYPE_H265 => CodecType::H265,
                                STREAM_TYPE_AAC => CodecType::AAC,
                                _ => continue,
                            };

                            let stream = StreamInfo {
                                stream_index: i,
                                config: Some(StreamCodecData {
                                    codec_type,
                                    width: None,
                                    height: None,
                                    extra_data: None,
                                }),
                                pes_buffer: None,
                            };
                            self.streams.insert(info.elementary_pid, stream);
                        }
                        self.pmt = Some(pmt);
                    }

                    elementary_pid if self.streams.contains_key(&elementary_pid) => {
                        let stream = self.streams.get_mut(&elementary_pid).unwrap();
                        let payload = &data[payload_offset..];

                        // Start new PES packet or add to existing
                        if header.payload_unit_start {
                            // Handle completed PES packet
                            if let Some(pes) = &mut stream.pes_buffer {
                                if !pes.data.is_empty() {
                                    let packet_data = pes.take_data();
                                    return Ok(Packet::new(Bytes::from(packet_data))
                                        .with_pts(pes.pts.unwrap_or(0))
                                        .with_stream_index(stream.stream_index));
                                }
                            }

                            // Parse PES header
                            if payload.len() >= 6 {
                                let packet_len = ((payload[4] as usize) << 8) | (payload[5] as usize);
                                let pts = parse_pts(payload);

                                // Create new PES packet
                                let mut pes = PESBuilder::new();
                                if let Some(pts) = pts {
                                    // Convert from 90kHz to ns
                                    pes.set_pts((pts as f64 * 1_000_000_000.0 / 90_000.0) as i64);
                                }
                                pes.set_size(packet_len);
                                stream.pes_buffer = Some(pes);
                            }
                        }

                        // Add payload to PES packet
                        if let Some(pes) = &mut stream.pes_buffer {
                            pes.push_data(payload);

                            // Return completed packets
                            if pes.is_complete() {
                                let packet_data = pes.take_data();
                                return Ok(Packet::new(Bytes::from(packet_data))
                                    .with_pts(pes.pts.unwrap_or(0))
                                    .with_stream_index(stream.stream_index));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    async fn streams(&mut self) -> Result<Vec<Box<dyn av::CodecDataExt>>> {
        // Read packets until we have stream information
        while self.pmt.is_none() {
            let data = self.read_packet_data().await?;
            let header = self.parser.parse_header(&data)?;

            // Skip packets with transport errors
            if header.transport_error {
                continue;
            }

            // Parse adaptation field if present
            let mut payload_offset = TS_HEADER_SIZE;
            if header.adaptation_field_exists {
                if let Some(adaptation) = self.parser.parse_adaptation_field(&data, payload_offset)? {
                    payload_offset += adaptation.length + 1;
                }
            }

            // Handle payload
            if header.contains_payload {
                match header.pid {
                    PID_PAT if header.payload_unit_start => {
                        // Parse PAT to get PMT PID
                        let table_offset = payload_offset + data[payload_offset] as usize + 1;
                        let pat = self.parser.parse_pat(&data[table_offset..], 0, 0)?;
                        
                        // Use first program's PMT
                        if let Some(entry) = pat.entries.first() {
                            self.pmt_pid = Some(entry.program_map_pid);
                        }
                    }

                    pmt_pid if Some(pmt_pid) == self.pmt_pid && header.payload_unit_start => {
                        // Parse PMT to get elementary stream info
                        let table_offset = payload_offset + data[payload_offset] as usize + 1;
                        let pmt = self.parser.parse_pmt(&data[table_offset..], 0, 0)?;
                        
                        // Create stream entries
                        for (i, info) in pmt.elementary_stream_infos.iter().enumerate() {
                            let codec_type = match info.stream_type {
                                STREAM_TYPE_H264 => CodecType::H264,
                                STREAM_TYPE_H265 => CodecType::H265,
                                STREAM_TYPE_AAC => CodecType::AAC,
                                _ => continue,
                            };

                            let stream = StreamInfo {
                                stream_index: i,
                                config: Some(StreamCodecData {
                                    codec_type,
                                    width: None,
                                    height: None,
                                    extra_data: None,
                                }),
                                pes_buffer: None,
                            };
                            self.streams.insert(info.elementary_pid, stream);
                        }
                        self.pmt = Some(pmt);
                        break;
                    }
                    _ => {}
                }
            }
        }

        // Return stream configurations
        let mut configs = Vec::new();
        for stream in self.streams.values() {
            if let Some(config) = &stream.config {
                // Create a Box<dyn CodecDataExt> directly
                let ext_config: Box<dyn av::CodecDataExt> = Box::new(config.clone());
                configs.push(ext_config);
            }
        }
        Ok(configs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::runtime::Runtime;

    fn create_pat_packet() -> Vec<u8> {
        let mut pat_packet = vec![0u8; TS_PACKET_SIZE];
        pat_packet[0] = 0x47; // Sync byte
        pat_packet[1] = 0x40; // Payload start indicator + no transport error
        pat_packet[2] = 0x00; // PID 0 (PAT)
        pat_packet[3] = 0x10; // No adaptation field
        pat_packet[4] = 0x00; // Pointer field
        pat_packet[5] = 0x00; // Table ID (PAT)
        
        // Section length (13 bytes)
        let section_length = 13;
        pat_packet[6] = 0xB0; // Section syntax indicator + length MSB
        pat_packet[7] = section_length as u8;

        pat_packet[8] = 0x00; // Transport stream ID high
        pat_packet[9] = 0x01; // Transport stream ID low
        pat_packet[10] = 0xC1; // Version (0) + current/next (1)
        pat_packet[11] = 0x00; // Section number
        pat_packet[12] = 0x00; // Last section number

        // Program entry (5 bytes)
        pat_packet[13] = 0x00; // Program number high
        pat_packet[14] = 0x01; // Program number low
        pat_packet[15] = 0xE0; // Reserved + PMT PID high (0x1000)
        pat_packet[16] = 0x20; // PMT PID low

        // Calculate and add CRC
        let mut crc = Crc32Mpeg2::new();
        let crc_val = crc.calculate(&pat_packet[5..17]);
        pat_packet[17] = (crc_val >> 24) as u8;
        pat_packet[18] = (crc_val >> 16) as u8;
        pat_packet[19] = (crc_val >> 8) as u8;
        pat_packet[20] = crc_val as u8;

        // Fill rest with stuffing bytes
        pat_packet[21..].fill(0xFF);
        pat_packet
    }

    fn create_pmt_packet() -> Vec<u8> {
        let mut pmt_packet = vec![0u8; TS_PACKET_SIZE];
        pmt_packet[0] = 0x47; // Sync byte
        pmt_packet[1] = 0x40; // Payload start + no error
        pmt_packet[2] = 0x20; // PID 0x1000 (PMT)
        pmt_packet[3] = 0x10; // No adaptation field
        pmt_packet[4] = 0x00; // Pointer field
        pmt_packet[5] = 0x02; // Table ID (PMT)

        // Section length (18 bytes = 13 base + 5 for one stream entry)
        let section_length = 18;
        pmt_packet[6] = 0xB0; // Section syntax + length MSB
        pmt_packet[7] = section_length as u8;

        pmt_packet[8] = 0x00; // Program number high
        pmt_packet[9] = 0x01; // Program number low
        pmt_packet[10] = 0xC1; // Version (0) + current

        pmt_packet[11] = 0x00; // Section number
        pmt_packet[12] = 0x00; // Last section number

        // PCR PID
        pmt_packet[13] = 0xE0; // Reserved + PCR PID high
        pmt_packet[14] = 0x21; // PCR PID low = 0x1001

        // Program info length (0)
        pmt_packet[15] = 0xF0; // Reserved + length high
        pmt_packet[16] = 0x00; // Length low

        // Stream entry
        pmt_packet[17] = STREAM_TYPE_H264; // Stream type
        pmt_packet[18] = 0xE0; // Reserved + elementary PID high
        pmt_packet[19] = 0x21; // Elementary PID low = 0x1001
        pmt_packet[20] = 0xF0; // Reserved + ES info length high
        pmt_packet[21] = 0x00; // ES info length low

        // Calculate and add CRC
        let mut crc = Crc32Mpeg2::new();
        let crc_val = crc.calculate(&pmt_packet[5..22]);
        pmt_packet[22] = (crc_val >> 24) as u8;
        pmt_packet[23] = (crc_val >> 16) as u8;
        pmt_packet[24] = (crc_val >> 8) as u8;
        pmt_packet[25] = crc_val as u8;

        // Fill rest with stuffing bytes
        pmt_packet[26..].fill(0xFF);
        pmt_packet
    }

    #[test]
    fn test_ts_demuxer_basic() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // Create test data with PAT and PMT
            let mut data = Vec::new();

            // Add PAT and PMT packets
            data.extend_from_slice(&create_pat_packet());
            data.extend_from_slice(&create_pmt_packet());

            // Create cursor with test data
            let mut demuxer = TSDemuxer::new(Cursor::new(data));
            let streams = demuxer.streams().await.unwrap();
            assert_eq!(streams.len(), 1);
            assert_eq!(streams[0].codec_type(), CodecType::H264);
        });
    }
}
