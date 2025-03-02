use super::hls::HLSSegmenter;
use super::types::*;
use crate::av::{self, CodecDataExt, Packet};
use crate::error::{Result, VdkError};
use crate::format::Muxer as FormatMuxer;
use crate::utils::crc::Crc32Mpeg2;
use bytes::{BufMut, BytesMut};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{self, AsyncWrite, AsyncWriteExt};

#[allow(dead_code)]
const PCR_INTERVAL: Duration = Duration::from_millis(40); // ~25 PCR updates per second

/// Codec information specific to Transport Stream format.
#[derive(Debug, Clone)]
struct TSCodecData {
    /// Type of codec (H264, AAC, etc.)
    codec_type: av::CodecType,
    /// Width in pixels for video codecs
    width: Option<u32>,
    /// Height in pixels for video codecs
    height: Option<u32>,
    /// Codec-specific extra data (e.g., SPS/PPS for H.264)
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

/// MPEG Transport Stream muxer.
///
/// Implements multiplexing of multiple elementary streams (video, audio)
/// into a single MPEG-TS bitstream. Supports:
/// - Multiple program streams
/// - Program Association Table (PAT) generation
/// - Program Map Table (PMT) generation
/// - Optional HLS segmentation
pub struct TSMuxer<W: AsyncWrite + Unpin + Send> {
    stream_writer: io::BufWriter<W>,
    streams: Vec<Box<dyn CodecDataExt>>,
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
    /// Creates a new Transport Stream muxer writing to the specified output.
    ///
    /// # Arguments
    ///
    /// * `writer` - The output writer where the TS data will be written
    pub fn new(writer: W) -> Self {
        Self {
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

    /// Configures HLS segmentation for this muxer.
    ///
    /// # Arguments
    ///
    /// * `segmenter` - The HLS segmenter configuration
    pub fn with_hls(mut self, segmenter: HLSSegmenter) -> Self {
        self.hls_segmenter = Some(segmenter);
        self
    }

    /// Marks the stream as discontinuous, affecting PCR and segment timing.
    pub fn mark_discontinuity(&mut self) {
        self.stream_discontinuity = true;
    }

    /// Resets the Program Clock Reference timing.
    pub fn reset_pcr(&mut self) {
        self.current_pcr = Duration::ZERO;
        self.last_pcr = None;
        self.last_pcr_write = Duration::ZERO;
    }

    /// Gets the PID for a stream index.
    fn get_stream_pid(&self, index: usize) -> u16 {
        0x100 + (index as u16)
    }

    /// Gets and increments the continuity counter for a stream.
    fn get_next_continuity_counter(&mut self, stream_index: usize) -> u8 {
        let counter = &mut self.continuity_counters[stream_index];
        *counter = (*counter + 1) & 0x0F;
        *counter
    }

    /// Updates the Program Clock Reference with new timing information.
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
}

#[async_trait::async_trait]
impl<W: AsyncWrite + Unpin + Send> FormatMuxer for TSMuxer<W> {
    /// Writes the initial Transport Stream headers including PAT and PMT.
    async fn write_header(&mut self, streams: &[Box<dyn CodecDataExt>]) -> Result<()> {
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
        let mut pat_buf = BytesMut::with_capacity(TS_PACKET_SIZE);

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
        header.write_to(&mut pat_buf)?;

        // Write PAT content
        pat_buf.put_u8(0); // Pointer field
        pat_buf.put_u8(TABLE_ID_PAT);

        let mut section = BytesMut::new();
        self.pat.write_to(&mut section)?;

        let section_length = section.len() + 5 + 4;
        pat_buf.put_u16((0xB000 | section_length as u16) & 0x3FF);
        pat_buf.put_u16(1); // Transport stream ID
        pat_buf.put_u8(0xC1); // Version 0, current

        pat_buf.put_u8(0); // Section number
        pat_buf.put_u8(0); // Last section number

        pat_buf.extend_from_slice(&section);

        // Calculate and write CRC
        let crc = self.crc.calculate(&pat_buf[5..5 + section_length - 4].to_vec());
        pat_buf.put_u32(crc);

        // Stuffing
        while pat_buf.len() < TS_PACKET_SIZE {
            pat_buf.put_u8(0xFF);
        }

        self.stream_writer.write_all(&pat_buf).await?;

        // Write PMT
        let mut pmt_buf = BytesMut::with_capacity(TS_PACKET_SIZE);

        // PMT header
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
        header.write_to(&mut pmt_buf)?;

        // Write PMT content
        pmt_buf.put_u8(0); // Pointer field
        pmt_buf.put_u8(TABLE_ID_PMT);

        let mut section = BytesMut::new();
        self.pmt.write_to(&mut section)?;

        let section_length = section.len() + 4 + 4; // +4 for program number and version, +4 for CRC
        pmt_buf.put_u16((0xB000 | section_length as u16) & 0x3FF);
        pmt_buf.put_u16(1); // Program number
        pmt_buf.put_u8(0xC1); // Version 0, current

        pmt_buf.put_u8(0); // Section number
        pmt_buf.put_u8(0); // Last section number

        pmt_buf.extend_from_slice(&section);

        // Calculate and write CRC
        let crc = self.crc.calculate(&pmt_buf[5..5 + section_length - 4].to_vec());
        pmt_buf.put_u32(crc);

        // Stuffing
        while pmt_buf.len() < TS_PACKET_SIZE {
            pmt_buf.put_u8(0xFF);
        }

        self.stream_writer.write_all(&pmt_buf).await?;
        self.stream_writer.flush().await?;

        Ok(())
    }

    /// Writes a media packet as one or more TS packets.
    async fn write_packet(&mut self, packet: &Packet) -> Result<()> {
        // Split packet data into TS packets
        let payload = &packet.data;
        let mut offset = 0;
        
        while offset < payload.len() {
            let mut ts_packet = BytesMut::with_capacity(TS_PACKET_SIZE);
                
            // Calculate sizes
            let header_size = 4;
            let adaptation_field_size = if offset == 0 { 1 } else { 0 }; // Only first packet has adaptation field
            let max_payload_size = TS_PACKET_SIZE - header_size - adaptation_field_size;
            let payload_size = std::cmp::min(max_payload_size, payload.len() - offset);
            let stuffing_size = TS_PACKET_SIZE - header_size - adaptation_field_size - payload_size;

            // Write TS header
            let header = TSHeader {
                sync_byte: 0x47,
                transport_error: false,
                payload_unit_start: offset == 0, // Only first packet has payload_unit_start
                transport_priority: false,
                pid: self.get_stream_pid(packet.stream_index),
                scrambling_control: 0,
                adaptation_field_exists: adaptation_field_size > 0 || stuffing_size > 0,
                contains_payload: true,
                continuity_counter: self.get_next_continuity_counter(packet.stream_index),
            };
            header.write_to(&mut ts_packet)?;

            // Write adaptation field if needed
            if header.adaptation_field_exists {
                ts_packet.put_u8((adaptation_field_size + stuffing_size) as u8); // Adaptation field length
                if adaptation_field_size > 0 {
                    ts_packet.put_u8(0); // No flags
                }
                // Add stuffing
                for _ in 0..stuffing_size {
                    ts_packet.put_u8(0xFF);
                }
            }
                
            // Write payload
            ts_packet.extend_from_slice(&payload[offset..offset + payload_size]);
                
            // Write packet
            self.stream_writer.write_all(&ts_packet).await?;
            offset += payload_size;
        }
            
        // Update PCR if needed
        if let Some(pts) = packet.pts {
            self.update_pcr(Some(Duration::from_millis(pts as u64)));
        }
            
        self.stream_writer.flush().await?;
        Ok(())
    }

    /// Finalizes the Transport Stream and writes any pending data.
    async fn write_trailer(&mut self) -> Result<()> {
        if let Some(segmenter) = &mut self.hls_segmenter {
            let current_time = self.current_pcr;
            segmenter.finish_segment(current_time).await?;

            let playlist_path = segmenter.get_output_dir().join("playlist.m3u8");
            let playlist_file = File::create(playlist_path).await?;
            let mut playlist_writer = io::BufWriter::new(playlist_file);

            segmenter.write_playlist(&mut playlist_writer).await?;
            segmenter.write_master_playlist(&mut playlist_writer).await?;
            playlist_writer.flush().await?;
        }

        self.stream_writer.flush().await?;
        Ok(())
    }

    /// Flushes any buffered data to the output.
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

    #[derive(Debug, Clone)]
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

    // CodecDataExt is automatically implemented for TestCodec since it implements
    // both CodecData and Clone (via #[derive(Clone)])

    #[test]
    fn test_ts_muxer_basic() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let buf = Vec::new();
            let mut muxer = TSMuxer::new(Cursor::new(buf));

            let streams = vec![Box::new(TestCodec) as Box<dyn CodecDataExt>];
            muxer.write_header(&streams).await.unwrap();

            let packet = Packet::new(bytes::Bytes::from(vec![0; 184]))
                .with_stream_index(0)
                .with_pts(0);
            muxer.write_packet(&packet).await.unwrap();
        });
    }
}
