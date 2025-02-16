use super::hls::HLSSegmenter;
use super::parser::TSPacketParser;
use super::types::*;
use crate::av::{self, Packet};
use crate::error::{Result, VdkError};
use crate::format::Muxer as FormatMuxer;
use crate::utils::crc::Crc32Mpeg2;
use bytes::{BufMut, BytesMut};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{self, AsyncWrite, AsyncWriteExt};

const PCR_INTERVAL: Duration = Duration::from_millis(40); // ~25 PCR updates per second

#[derive(Clone)]
struct TSCodecData {
    codec_type: av::CodecType,
    width: Option<u32>,
    height: Option<u32>,
    extra_data: Option<Vec<u8>>,
}

impl av::CodecData for TSCodecData {
    fn codec_type(&self) -> av::CodecType {
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

pub struct TSMuxer<W: AsyncWrite + Unpin + Send> {
    parser: TSPacketParser,
    stream_writer: io::BufWriter<W>,
    streams: Vec<Box<dyn av::CodecData>>,
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
        Self {
            parser: TSPacketParser::new(),
            stream_writer: io::BufWriter::new(writer),
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

#[async_trait::async_trait]
impl<W: AsyncWrite + Unpin + Send> FormatMuxer for TSMuxer<W> {
    async fn write_header(&mut self, streams: &[Box<dyn av::CodecData>]) -> Result<()> {
        // Initialize PAT
        self.pat.entries.clear();
        self.pat.entries.push(PATEntry {
            program_number: 1,
            network_pid: 0,
            program_map_pid: PID_PMT,
        });

        // Set PCR PID to first stream's PID if available
        if !streams.is_empty() {
            self.pmt.pcr_pid = self.get_stream_pid(0);
        }

        // Initialize streams
        for codec in streams {
            let stream_type = match codec.codec_type() {
                av::CodecType::H264 => STREAM_TYPE_H264,
                av::CodecType::H265 => STREAM_TYPE_H265,
                av::CodecType::AAC => STREAM_TYPE_AAC,
                _ => return Err(VdkError::InvalidData("Unsupported codec type".to_string())),
            };

            let elementary_pid = 0x100 + (self.streams.len() as u16);
            self.pmt.elementary_stream_infos.push(ElementaryStreamInfo {
                stream_type,
                elementary_pid,
                descriptors: Vec::new(),
            });

            self.continuity_counters.push(0);
            self.streams.push(Box::new(TSCodecData {
                codec_type: codec.codec_type(),
                width: codec.width(),
                height: codec.height(),
                extra_data: codec.extra_data().map(|d| d.to_vec()),
            }));
        }

        // Write PAT
        let mut buf = BytesMut::with_capacity(TS_PACKET_SIZE);

        // PAT header
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

        // Write PAT content
        buf.put_u8(0); // Pointer field
        buf.put_u8(TABLE_ID_PAT);

        let mut section = BytesMut::new();
        self.pat.write_to(&mut section)?;

        let section_length = section.len() + 5 + 4;
        buf.put_u16((0xB000 | section_length as u16) & 0x3FF);
        buf.put_u16(1); // Transport stream ID
        buf.put_u8(0xC1); // Version 0, current

        buf.put_u8(0); // Section number
        buf.put_u8(0); // Last section number

        buf.extend_from_slice(&section);

        // Calculate and write CRC
        let crc = self.crc.calculate(&buf[5..5 + section_length - 4].to_vec());
        buf.put_u32(crc);

        // Stuffing
        while buf.len() < TS_PACKET_SIZE {
            buf.put_u8(0xFF);
        }

        self.stream_writer.write_all(&buf).await?;
        self.stream_writer.flush().await?;

        Ok(())
    }

    async fn write_packet(&mut self, packet: &Packet) -> Result<()> {
        let mut buf = BytesMut::with_capacity(TS_PACKET_SIZE);

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
        let header_size = 4;
        let stuffing_needed = if payload_size + header_size + adaptation_size < TS_PACKET_SIZE {
            TS_PACKET_SIZE - (payload_size + header_size + adaptation_size)
        } else {
            0
        };

        // Write TS header
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
            let mut adaptation_length = stuffing_needed;
            if need_pcr {
                adaptation_length += 7;
            }
            if self.stream_discontinuity {
                adaptation_length += 1;
            }

            // Adaptation field length
            buf.put_u8(adaptation_length as u8);

            // Adaptation flags
            let mut flags = 0u8;
            if need_pcr {
                flags |= 0x10; // PCR flag
            }
            if self.stream_discontinuity {
                flags |= 0x80; // Discontinuity indicator
            }
            if stuffing_needed > 0 {
                flags |= 0x20; // Random access indicator
            }
            buf.put_u8(flags);

            // Write PCR if needed
            if need_pcr {
                let pcr = time_to_pcr(self.current_pcr);
                buf.extend_from_slice(&((pcr >> 16) as u32).to_be_bytes());
                buf.extend_from_slice(&((pcr & 0xFFFF) as u16).to_be_bytes());
                self.last_pcr = Some(self.current_pcr);
                self.last_pcr_write = self.current_pcr;
            }

            // Stuffing bytes
            for _ in 0..stuffing_needed {
                buf.put_u8(0xFF);
            }
        }

        // Write payload
        buf.extend_from_slice(&packet.data);

        if let Some(segmenter) = &mut self.hls_segmenter {
            let current_time = Duration::from_millis(packet.pts.unwrap_or(0) as u64); // Use packet PTS as time
            if segmenter.should_start_new_segment(current_time) {
                if let Err(e) = segmenter.finish_segment(current_time).await {
                    println!("Error finishing segment: {}", e); // Log error but continue
                }
                if let Err(e) = segmenter.start_segment(current_time).await {
                    return Err(VdkError::Codec(format!("Failed to start segment: {}", e)));
                }
            }
        }

        self.stream_writer.write_all(&buf).await?;
        self.stream_writer.flush().await?;
        Ok(())
    }

    
        async fn write_trailer(&mut self) -> Result<()> {
            if let Some(segmenter) = &mut self.hls_segmenter {
                let current_time = self.current_pcr; // Use current PCR time for segment end
                if let Err(e) = segmenter.finish_segment(current_time).await {
                    println!("Error finishing last segment: {}", e); // Log error but continue
                }
    
                let playlist_path = segmenter.get_output_dir().join("playlist.m3u8");
                let playlist_file = File::create(playlist_path).await?;
                let mut playlist_writer = io::BufWriter::new(playlist_file);
    
                if let Err(e) = segmenter.write_playlist(&mut playlist_writer).await {
                    println!("Error writing playlist: {}", e); // Log error but continue
                }
                if let Err(e) = segmenter.write_master_playlist(&mut playlist_writer).await {
                    println!("Error writing master playlist: {}", e); // Log error but continue
                }
    
                if let Err(e) = playlist_writer.flush().await {
                    println!("Error flushing playlist writer: {}", e); // Log error but continue
                }
            }
    
            self.stream_writer.flush().await?;
            Ok(())
        }
    async fn flush(&mut self) -> Result<()> {
        self.stream_writer.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
   use std::io::Cursor;
    use tokio::runtime::Runtime;

    #[derive(Clone)]
    struct TestCodec;

    impl av::CodecData for TestCodec {
        fn codec_type(&self) -> av::CodecType {
            av::CodecType::H264
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
            let buf = Vec::new();
            let mut muxer = TSMuxer::new(Cursor::new(buf));

            // Cast TestCodec as CodecData instead of CodecDataExt
            let streams = vec![Box::new(TestCodec) as Box<dyn av::CodecData>];
            muxer.write_header(&streams).await.unwrap();

            // Create test packet
            let packet = Packet::new(bytes::Bytes::from(vec![0; 184]))
                .with_stream_index(0)
                .with_pts(0);
            muxer.write_packet(&packet).await.unwrap();
        });
    }
}
