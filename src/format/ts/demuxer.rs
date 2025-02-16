use super::{parser::TSPacketParser, types::*};
use crate::av::{self, CodecData, CodecType, Packet};
use crate::error::{Result, VdkError};
use crate::format;
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

#[derive(Clone)]
struct TSCodecData {
    codec_type: CodecType,
    width: Option<u32>,
    height: Option<u32>,
    extra_data: Option<Vec<u8>>,
}

#[async_trait]
impl CodecData for TSCodecData {
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

#[derive(Default)]
struct PESState {
    buffer: BytesMut,
    stream_id: u8,
    packet_length: usize,
    has_length: bool,
    pts: Option<i64>,
}

impl PESState {
    fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
            stream_id: 0,
            packet_length: 0,
            has_length: false,
            pts: None,
        }
    }

    fn reset(&mut self) {
        self.buffer.clear();
        self.stream_id = 0;
        self.packet_length = 0;
        self.has_length = false;
        self.pts = None;
    }

    fn packet_complete(&self) -> bool {
        if !self.has_length {
            return false;
        }
        self.buffer.len() >= self.packet_length
    }

    fn take_packet(&mut self) -> Option<(Bytes, i64)> {
        if self.packet_complete() {
            let pts = self.pts.unwrap_or(0);
            let data = if self.packet_length > 0 {
                self.buffer.split_to(self.packet_length).freeze()
            } else {
                self.buffer.split().freeze()
            };
            self.reset();
            Some((data, pts))
        } else {
            None
        }
    }
}

pub struct TSDemuxer<R: AsyncRead + Unpin + Send> {
    reader: BufReader<R>,
    parser: TSPacketParser,
    pat: Option<PAT>,
    pmt: Option<PMT>,
    streams: Vec<Arc<TSCodecData>>,
    pids_to_stream_indices: Vec<(u16, usize)>,
    pes_states: Vec<PESState>,
}

impl<R: AsyncRead + Unpin + Send> TSDemuxer<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            parser: TSPacketParser::new(),
            pat: None,
            pmt: None,
            streams: Vec::new(),
            pids_to_stream_indices: Vec::new(),
            pes_states: Vec::new(),
        }
    }

    fn get_pes_state(&mut self, stream_idx: usize) -> &mut PESState {
        while self.pes_states.len() <= stream_idx {
            self.pes_states.push(PESState::new());
        }
        &mut self.pes_states[stream_idx]
    }

    fn reset_pes_states(&mut self) {
        self.pes_states.clear();
    }

    async fn read_ts_packet(&mut self) -> Result<Option<Packet>> {
        let mut data = BytesMut::with_capacity(TS_PACKET_SIZE);
        data.resize(TS_PACKET_SIZE, 0);

        // Read until we find sync byte
        loop {
            // Read one byte at a time until we find sync byte
            let mut sync = [0u8; 1];
            if let Err(e) = self.reader.read_exact(&mut sync).await {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return Ok(None);
                }
                return Err(VdkError::Io(e));
            }

            if sync[0] == 0x47 {
                // Found sync byte, read rest of packet
                data[0] = sync[0];
                if let Err(e) = self.reader.read_exact(&mut data[1..]).await {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        return Ok(None);
                    }
                    return Err(VdkError::Io(e));
                }
                break;
            }
        }

        println!("Read TS packet: first 4 bytes: {:02x} {:02x} {:02x} {:02x}", data[0], data[1], data[2], data[3]);

        let header = self.parser.parse_header(&data)?;
        println!("  Parsed TS header: PID=0x{:04x}, payload_start={}, continuity_counter={}",
                 header.pid, header.payload_unit_start, header.continuity_counter);
        let adaptation_field = self.parser.parse_adaptation_field(&data, TS_HEADER_SIZE)?;
        if let Some(ref af) = adaptation_field {
            println!("  Adaptation field: length={}, pcr_flag={}", af.length, af.pcr_flag);
        }

        let payload_offset = TS_HEADER_SIZE + adaptation_field.map(|f| f.length + 1).unwrap_or(0);
        let mut effective_payload = &data[payload_offset..];

        // Handle pointer field for PSI sections (PAT/PMT)
        if header.payload_unit_start && (header.pid == PID_PAT || header.pid == PID_PMT) {
            let pointer_field = effective_payload[0] as usize;
            effective_payload = &effective_payload[1 + pointer_field..];
        }

        println!("Processing PID 0x{:04x}, payload_start={}, len={}",
                header.pid, header.payload_unit_start, effective_payload.len());

        match header.pid {
            PID_PAT => {
                self.pat = Some(self.parser.parse_pat(effective_payload, 0, effective_payload.len())?);
                println!("  Parsed PAT with {} entries", self.pat.as_ref().unwrap().entries.len());
                Ok(None)
            }
            PID_PMT => {
                if let Some(pat) = &self.pat {
                    for entry in &pat.entries {
                        if entry.program_number != 0 && header.pid == entry.program_map_pid {
                            self.pmt = Some(self.parser.parse_pmt(effective_payload, 0, effective_payload.len())?);
                            println!("  Parsed PMT with {} streams", self.pmt.as_ref().unwrap().elementary_stream_infos.len());
                            self.setup_streams()?;
                            self.reset_pes_states(); // Reset PES states when stream setup changes
                            break;
                        }
                    }
                }
                Ok(None)
            }
            _ => {
                if let Some(stream_idx) = self.get_stream_index(header.pid) {
                    println!("  Found stream {} for PID 0x{:04x}", stream_idx, header.pid);
                    let state = self.get_pes_state(stream_idx);

                    if header.payload_unit_start {
                        // Return any previous packet first
                        if let Some((data, pts)) = state.take_packet() {
                            return Ok(Some(Packet::new(data)
                                .with_stream_index(stream_idx)
                                .with_key_flag(true)
                                .with_pts(pts)));
                        }

                        // Start new PES packet
                        if effective_payload.len() < 6 {
                            println!("  PES header too short");
                            return Ok(None);
                        }

                        // Check PES start code
                        if effective_payload[0] != 0 || effective_payload[1] != 0 || effective_payload[2] != 1 {
                            println!("  Invalid PES start code: {:02x} {:02x} {:02x}",
                                   effective_payload[0], effective_payload[1], effective_payload[2]);
                            return Ok(None);
                        }

                        state.stream_id = effective_payload[3];
                        state.packet_length = ((effective_payload[4] as usize) << 8) | effective_payload[5] as usize;
                        state.has_length = true;

                        println!("  New PES: stream_id=0x{:02x}, length={}", state.stream_id, state.packet_length);

                        // Parse optional PES header fields
                        let mut payload_offset = 6;
                        if effective_payload.len() > 9 {
                            let pts_dts_flags = (effective_payload[7] >> 6) & 0x03;
                            let header_length = effective_payload[8] as usize;

                            // Parse PTS if present
                            if pts_dts_flags > 0 && effective_payload.len() >= 9 + header_length {
                                if (effective_payload[9] & 0xF0) == 0x20 || (effective_payload[9] & 0xF0) == 0x30 {
                                    let pts = ((effective_payload[9] as i64 & 0x0E) << 29) |
                                            ((effective_payload[10] as i64) << 22) |
                                            ((effective_payload[11] as i64 & 0xFE) << 14) |
                                            ((effective_payload[12] as i64) << 7) |
                                            ((effective_payload[13] as i64) >> 1);
                                    state.pts = Some(pts);
                                    println!("  PTS: {}", pts);
                                }
                            }
                            payload_offset += 3 + header_length;
                        }

                        // Add payload data after header
                        if payload_offset < effective_payload.len() {
                            state.buffer.extend_from_slice(&effective_payload[payload_offset..]);
                        }
                    } else {
                        // Continuation packet
                        state.buffer.extend_from_slice(effective_payload);
                    }

                    // Check if we have a complete packet
                    if let Some((data, pts)) = state.take_packet() {
                        // Ensure streams are set up before returning packets
                        if self.streams.is_empty() && self.pmt.is_some() {
                            self.setup_streams()?;
                        }
                        
                        // Get the codec type for this stream
                        let is_key = if let Some(pmt) = &self.pmt {
                            pmt.elementary_stream_infos
                                .iter()
                                .find(|info| info.elementary_pid == header.pid)
                                .map(|info| info.stream_type == STREAM_TYPE_H264 || info.stream_type == STREAM_TYPE_H265)
                                .unwrap_or(false)
                        } else {
                            false
                        };

                        Ok(Some(Packet::new(data)
                            .with_stream_index(stream_idx)
                            .with_key_flag(is_key)
                            .with_pts(pts)))
                    } else {
                        Ok(None)
                    }
                } else {
                    println!("  No stream found for PID 0x{:04x}", header.pid);
                    Ok(None)
                }
            }
        }
    }

    fn setup_streams(&mut self) -> Result<()> {
        if let Some(pmt) = &self.pmt {
            self.streams.clear();
            self.pids_to_stream_indices.clear();
            self.pes_states.clear();

            // First pass: collect all video and audio streams
            let mut video_streams = Vec::new();
            let mut audio_streams = Vec::new();

            for info in &pmt.elementary_stream_infos {
                match info.stream_type {
                    STREAM_TYPE_H264 | STREAM_TYPE_H265 => video_streams.push(info),
                    STREAM_TYPE_AAC => audio_streams.push(info),
                    _ => continue, // Skip unsupported stream types
                }
            }

            // Add video streams first
            for info in video_streams {
                let codec_type = if info.stream_type == STREAM_TYPE_H264 {
                    CodecType::H264
                } else {
                    CodecType::H265
                };

                let idx = self.streams.len();
                self.streams.push(Arc::new(TSCodecData {
                    codec_type,
                    width: None,
                    height: None,
                    extra_data: None,
                }));
                self.pids_to_stream_indices.push((info.elementary_pid, idx));
                println!("Added video stream: PID=0x{:04x}, index={}", info.elementary_pid, idx);
            }

            // Then add audio streams
            for info in audio_streams {
                let idx = self.streams.len();
                self.streams.push(Arc::new(TSCodecData {
                    codec_type: CodecType::AAC,
                    width: None,
                    height: None,
                    extra_data: None,
                }));
                self.pids_to_stream_indices.push((info.elementary_pid, idx));
                println!("Added audio stream: PID=0x{:04x}, index={}", info.elementary_pid, idx);
            }
        }
        Ok(())
    }

    fn get_stream_index(&self, pid: u16) -> Option<usize> {
        self.pids_to_stream_indices
            .iter()
            .find(|(p, _)| *p == pid)
            .map(|(_, idx)| *idx)
    }
}

#[async_trait]
impl<R: AsyncRead + Unpin + Send> format::Demuxer for TSDemuxer<R> {
    async fn read_packet(&mut self) -> Result<Packet> {
        loop {
            if let Some(packet) = self.read_ts_packet().await? {
                return Ok(packet);
            }
        }
    }

    async fn streams(&mut self) -> Result<Vec<Box<dyn av::CodecData>>> {
        // Keep reading packets until we have both PAT and PMT
        while self.pat.is_none() || self.pmt.is_none() {
            if self.read_ts_packet().await?.is_none() {
                break;
            }
        }

        // Setup streams if we haven't yet
        if self.streams.is_empty() && self.pmt.is_some() {
            self.setup_streams()?;
        }

        if self.streams.is_empty() {
            println!("No streams found in TS file");
            println!("PAT entries: {}", self.pat.as_ref().map_or(0, |pat| pat.entries.len()));
            println!("PMT streams: {}", self.pmt.as_ref().map_or(0, |pmt| pmt.elementary_stream_infos.len()));
        }

        Ok(self
            .streams
            .iter()
            .map(|s| {
                let codec = TSCodecData {
                    codec_type: s.codec_type,
                    width: s.width,
                    height: s.height,
                    extra_data: s.extra_data.clone(),
                };
                Box::new(codec) as Box<dyn av::CodecData>
            })
            .collect())
    }
}
