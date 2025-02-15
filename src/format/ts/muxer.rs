use tokio::io::{AsyncWrite, AsyncWriteExt};
use crate::av::{Packet, CodecData, Muxer};
use crate::error::{Result, VdkError};
use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::time::Duration;
use super::types::*;
use super::parser::TSPacketParser;
use super::parser::TSPacketParser;
use super::hls::HLSSegmenter;
use crate::utils::crc::Crc32Mpeg2;

const PCR_INTERVAL: Duration = Duration::from_millis(40); // ~25 PCR updates per second

pub struct TSMuxer<W: AsyncWrite + Unpin + Send> {
    parser: TSPacketParser,
    stream_writer: tokio::io::BufWriter<W>,
    streams: Vec<Box<dyn CodecData>>,
    continuity_counters: Vec<u8>,
    current_pcr: Duration,
    last_pcr: Option<Duration>,
    last_pcr_write: Duration,
    pat: PAT,
    pmt: PMT,
    crc: Crc32Mpeg2,
    hls_segmenter: Option<HLSSegmenter>,
    stream_discontinuity: bool,
}

impl<W: AsyncWrite + Unpin + Send> TSMuxer<W> {
    pub fn new(writer: W) -> Self {
        parser: TSPacketParser::new(),
            stream_writer: tokio::io::BufWriter::new(writer),
            parser: TSPacketParser::new(),
            streams: Vec::new(),
            continuity_counters: Vec::new(),
            current_pcr: Duration::ZERO,
            last_pcr: None,
            last_pcr_write: Duration::ZERO,
            pat: PAT::new(),
            pmt: PMT::new(),
            crc: Crc32Mpeg2::new(),
            hls_segmenter: None,
            stream_discontinuity: false,
        }
    }

    pub fn with_hls(mut self, segmenter: HLSSegmenter) -> Self {
        self.hls_segmenter = Some(segmenter);
        self
    }

    pub fn mark_discontinuity(&mut self) {
        self.stream_discontinuity = true;
    }

    pub fn reset_pcr(&mut self) {
        self.current_pcr = Duration::ZERO;
        self.last_pcr = None;
        self.last_pcr_write = Duration::ZERO;
    }

    pub async fn add_stream(&mut self, codec: Box<dyn CodecData>) -> Result<()> {
        let stream_type = match codec.codec_type() {
            crate::av::CodecType::H264 => STREAM_TYPE_H264,
            crate::av::CodecType::H265 => STREAM_TYPE_H265,
            crate::av::CodecType::AAC => STREAM_TYPE_AAC,
            _ => return Err(VdkError::InvalidData("Unsupported codec type".to_string())),
        };

        let elementary_pid = 0x100 + (self.streams.len() as u16);

        // Add to PMT
        self.pmt.elementary_stream_infos.push(ElementaryStreamInfo {
            stream_type,
            elementary_pid,
            descriptors: Vec::new(),
        });

        self.continuity_counters.push(0);
        self.streams.push(codec);
        Ok(())
    }

    fn write_adaptation_field(&mut self, buf: &mut BytesMut, need_pcr: bool, stuffing_bytes: usize) -> Result<()> {
        if !need_pcr && stuffing_bytes == 0 && !self.stream_discontinuity {
            return Ok(());
        }

        // Calculate adaptation field length
        let mut adaptation_length = if need_pcr { 7 } else { 0 } + stuffing_bytes;
        if self.stream_discontinuity {
            adaptation_length += 1;
        }

        buf.put_u8(adaptation_length as u8);

        // Set adaptation flags
        let mut flags = 0u8;
        if need_pcr {
            flags |= 0x10; // PCR flag
        }
        if self.stream_discontinuity {
            flags |= 0x80; // Discontinuity indicator
        }
        if stuffing_bytes > 0 {
            flags |= 0x20; // Random access indicator
        }
        buf.put_u8(flags);

        // Write PCR if needed
        if need_pcr {
            let pcr = time_to_pcr(self.current_pcr);
            buf.extend_from_slice(&((pcr >> 16) as u32).to_be_bytes());
            buf.extend_from_slice(&((pcr & 0xFFFF) as u16).to_be_bytes());
            self.last_pcr = Some(self.current_pcr);
        }

        // Add stuffing bytes
        for _ in 0..stuffing_bytes {
            buf.put_u8(0xFF);
        }

        // Reset discontinuity flag after writing
        self.stream_discontinuity = false;

        Ok(())
    }

    async fn write_pat(&mut self) -> Result<()> {
        let mut buf = BytesMut::with_capacity(TS_PACKET_SIZE);

        // TS header
        let header = TSHeader {
            sync_byte: 0x47,
            transport_error: false,
            payload_unit_start: true,
            transport_priority: false,
            pid: PID_PAT,
            scrambling_control: 0,
            adaptation_field_exists: false,
            contains_payload: true,
            continuity_counter: 0,
        };
        header.write_to(&mut buf)?;

        // Pointer field
        buf.put_u8(0);

        // Table ID
        buf.put_u8(TABLE_ID_PAT);

        // Initialize section data
        let mut section = BytesMut::new();

        // Write PAT entries
        self.pat.write_to(&mut section)?;

        // Calculate section length (PAT length + 5 bytes header + 4 bytes CRC)
        let section_length = section.len() + 5 + 4;
        buf.put_u16((0xB000 | section_length as u16) & 0x3FF);

        // Transport stream ID
        buf.put_u16(1);

        // Version and current/next indicator
        buf.put_u8(0xC1); // Version 0, current

        // Section number and last section number
        buf.put_u8(0);
        buf.put_u8(0);

        // Write section data
        buf.extend_from_slice(&section);

        // Calculate and write CRC32
        let crc = self.crc.calculate(&buf[5..5+section_length-4].to_vec());
        buf.put_u32(crc);

        // Fill remainder with stuffing bytes
        while buf.len() < TS_PACKET_SIZE {
            buf.put_u8(0xFF);
        }

        self.writer.write_all(&buf).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn write_pmt(&mut self) -> Result<()> {
        let mut buf = BytesMut::with_capacity(TS_PACKET_SIZE);

        // TS header
        let header = TSHeader {
            sync_byte: 0x47,
            transport_error: false,
            payload_unit_start: true,
            transport_priority: false,
            pid: PID_PMT,
            scrambling_control: 0,
            adaptation_field_exists: false,
            contains_payload: true,
            continuity_counter: 0,
        };
        header.write_to(&mut buf)?;

        // Pointer field
        buf.put_u8(0);

        // Table ID
        buf.put_u8(TABLE_ID_PMT);

        // Initialize section data
        let mut section = BytesMut::new();

        // Write PMT content
        self.pmt.write_to(&mut section)?;

        // Calculate section length (PMT length + 5 bytes header + 4 bytes CRC)
        let section_length = section.len() + 5 + 4;
        buf.put_u16((0xB000 | section_length as u16) & 0x3FF);

        // Program number
        buf.put_u16(1);

        // Version and current/next indicator
        buf.put_u8(0xC1); // Version 0, current

        // Section number and last section number
        buf.put_u8(0);
        buf.put_u8(0);

        // Write section data
        buf.extend_from_slice(&section);

        // Calculate and write CRC32
        let crc = self.crc.calculate(&buf[5..5+section_length-4].to_vec());
        buf.put_u32(crc);

        // Fill remainder with stuffing bytes
        while buf.len() < TS_PACKET_SIZE {
            buf.put_u8(0xFF);
        }

        self.writer.write_all(&buf).await?;
        self.writer.flush().await?;
        Ok(())
    }

    pub(crate) fn get_stream_pid(&self, index: usize) -> u16 {
        0x100 + (index as u16)
    }

    pub(crate) fn get_next_continuity_counter(&mut self, stream_index: usize) -> u8 {
        let counter = &mut self.continuity_counters[stream_index];
        *counter = (*counter + 1) & 0x0F;
        *counter
    }

    fn update_pcr(&mut self, packet_time: Option<Duration>) {
        if let Some(time) = packet_time {
            if let Some(last_pcr) = self.last_pcr {
                if time < last_pcr {
                    // PCR regression detected, mark discontinuity
                    self.mark_discontinuity();
                }
            }
            self.current_pcr = time;
        }

    fn needs_pcr(&self) -> bool {
        self.current_pcr >= self.last_pcr_write + PCR_INTERVAL
    }
}

#[async_trait]
impl<W: AsyncWrite + Unpin + Send> Muxer for TSMuxer<W> {
    async fn write_header(&mut self, _streams: &[Box<dyn CodecData>]) -> Result<()> {
        // Add single program to PAT
        self.pat.entries.clear();
        self.pat.entries.push(PATEntry {
            program_number: 1,
            network_pid: 0,
            program_map_pid: PID_PMT,
        });

        // Set PCR PID to the first stream's PID
        if !self.streams.is_empty() {
            self.pmt.pcr_pid = self.get_stream_pid(0);
        }

        // Write PAT and PMT
        self.write_pat().await?;
        self.write_pmt().await?;

        // Reset PCR and discontinuity state
        self.reset_pcr();
        self.stream_discontinuity = false;

        Ok(())
            // PCR regression detected, mark discontinuity
                    self.mark_discontinuity();
                }
            }
            self.current_pcr = time;
        }
    }

    fn needs_pcr(&self) -> bool {
        self.current_pcr >= self.last_pcr_write + PCR_INTERVAL
    }
}

#[async_trait]
impl<W: AsyncWrite + Unpin + Send> Muxer for TSMuxer<W> {
    async fn write_header(&mut self, _streams: &[Box<dyn CodecData>]) -> Result<()> {
        // Add single program to PAT
        self.pat.entries.clear();
        self.pat.entries.push(PATEntry {
            program_number: 1,
            network_pid: 0,
            program_map_pid: PID_PMT,
        });

        // Set PCR PID to the first stream's PID
        if !self.streams.is_empty() {
            self.pmt.pcr_pid = self.get_stream_pid(0);
        }

        // Write PAT and PMT
        self.write_pat().await?;
        self.write_pmt().await?;

        // Reset PCR and discontinuity state
        self.reset_pcr();
        self.stream_discontinuity = false;

        Ok(())
    }

    async fn write_packet(&mut self, packet: Packet) -> Result<()> {
        let mut buf = BytesMut::with_capacity(TS_PACKET_SIZE);

        // Update PCR
        if let Some(pts) = packet.pts {
            self.update_pcr(Some(Duration::from_millis(pts as u64)));
        }

        let need_pcr = self.needs_pcr() && packet.stream_index == 0;
        let is_pcr_pid = self.get_stream_pid(packet.stream_index) == self.pmt.pcr_pid;

        // Calculate adaptation field size
        let mut adaptation_size = if need_pcr && is_pcr_pid { 8 } else { 0 };
        if self.stream_discontinuity {
            adaptation_size += 1;
        }

        let payload_size = packet.data.len();
        let header_size = 4; // Base TS header size
        let stuffing_needed = if payload_size + header_size + adaptation_size < TS_PACKET_SIZE {
            TS_PACKET_SIZE - (payload_size + header_size + adaptation_size)
        } else {
            0
        };

        // TS Header
        let header = TSHeader {
            sync_byte: 0x47,
            transport_error: false,
            payload_unit_start: true,
            transport_priority: false,
            pid: self.get_stream_pid(packet.stream_index),
            scrambling_control: 0,
            adaptation_field_exists: need_pcr || stuffing_needed > 0 || self.stream_discontinuity,
            contains_payload: true,
            continuity_counter: self.get_next_continuity_counter(packet.stream_index),
        };
        header.write_to(&mut buf)?;

        // Write adaptation field if needed
        if header.adaptation_field_exists {
            self.write_adaptation_field(&mut buf, need_pcr && is_pcr_pid, stuffing_needed)?;
            if need_pcr {
                self.last_pcr_write = self.current_pcr;
            }
        }

        // Write packet data
        buf.extend_from_slice(&packet.data);

        // Fill remainder with stuffing bytes if needed
        while buf.len() < TS_PACKET_SIZE {
            buf.put_u8(0xFF);
        }

        self.writer.write_all(&buf).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn write_trailer(&mut self) -> Result<()> {
        self.writer.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::runtime::Runtime;

    struct TestCodec;
    impl CodecData for TestCodec {
        fn codec_type(&self) -> crate::av::CodecType {
            crate::av::CodecType::H264
        }
        fn width(&self) -> Option<u32> {
            None
        }
        fn height(&self) -> Option<u32> {
            None
        }
        fn extra_data(&self) -> Option<&[u8]> {
            None
        }
    }

    #[test]
    fn test_ts_muxer_basic() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let data = Vec::new();
            let mut muxer = TSMuxer::new(Cursor::new(data));

            // Add a stream
            muxer.add_stream(Box::new(TestCodec)).await.unwrap();
            let streams = vec![Box::new(TestCodec) as Box<dyn CodecData>];
            muxer.write_header(&streams).await.unwrap();

            // Create test packet
            let mut packet = Packet::new(vec![0; 184]);
            packet.stream_index = 0;
            packet.pts = Some(0);
            packet.dts = Some(0);
            muxer.write_packet(packet).await.unwrap();
        });
    }

    #[test]
    fn test_ts_muxer_pcr_discontinuity() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let data = Vec::new();
            let mut muxer = TSMuxer::new(Cursor::new(data));
            muxer.add_stream(Box::new(TestCodec)).await.unwrap();

            let streams = vec![Box::new(TestCodec) as Box<dyn CodecData>];
            muxer.write_header(&streams).await.unwrap();

            // Write packet with normal PCR
            let mut packet = Packet::new(vec![0; 184]);
            packet.stream_index = 0;
            packet.pts = Some(1000);
            muxer.write_packet(packet.clone()).await.unwrap();

            // Write packet with regressed PCR (should trigger discontinuity)
            packet.pts = Some(500);
            muxer.write_packet(packet).await.unwrap();
        });
    }
}
