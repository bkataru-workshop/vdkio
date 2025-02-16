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

pub struct TSDemuxer<R: AsyncRead + Unpin + Send> {
    reader: BufReader<R>,
    parser: TSPacketParser,
    pat: Option<PAT>,
    pmt: Option<PMT>,
    streams: Vec<Arc<TSCodecData>>,
    pids_to_stream_indices: Vec<(u16, usize)>,
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
        }
    }

    async fn read_ts_packet(&mut self) -> Result<Option<Packet>> {
        let mut data = BytesMut::with_capacity(TS_PACKET_SIZE);
        data.resize(TS_PACKET_SIZE, 0);

        // Read TS packet
        if let Err(e) = self.reader.read_exact(&mut data).await {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(VdkError::Io(e));
        }

        let header = self.parser.parse_header(&data)?;
        let adaptation_field = self.parser.parse_adaptation_field(&data, TS_HEADER_SIZE)?;

        let payload_offset = TS_HEADER_SIZE + adaptation_field.map(|f| f.length + 1).unwrap_or(0);

        let payload = &data[payload_offset..];

        match header.pid {
            PID_PAT => {
                self.pat = Some(self.parser.parse_pat(payload, 0, payload.len())?);
                Ok(None)
            }
            PID_PMT => {
                if let Some(pat) = &self.pat {
                    for entry in &pat.entries {
                        if entry.program_number != 0 && header.pid == entry.program_map_pid {
                            self.pmt = Some(self.parser.parse_pmt(payload, 0, payload.len())?);
                            self.setup_streams()?;
                            break;
                        }
                    }
                }
                Ok(None)
            }
            _ => {
                if let Some(stream_idx) = self.get_stream_index(header.pid) {
                    let mut packet = Packet::new(Bytes::from(payload.to_vec()))
                        .with_stream_index(stream_idx)
                        .with_key_flag(header.payload_unit_start);

                    if header.payload_unit_start {
                        // TODO: Parse PES header and set pts/dts
                        packet = packet.with_pts(0);
                    }

                    Ok(Some(packet))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn setup_streams(&mut self) -> Result<()> {
        if let Some(pmt) = &self.pmt {
            self.streams.clear();
            self.pids_to_stream_indices.clear();

            for (idx, info) in pmt.elementary_stream_infos.iter().enumerate() {
                let codec_type = match info.stream_type {
                    STREAM_TYPE_H264 => CodecType::H264,
                    STREAM_TYPE_H265 => CodecType::H265,
                    STREAM_TYPE_AAC => CodecType::AAC,
                    _ => continue, // Skip unsupported stream types
                };

                self.streams.push(Arc::new(TSCodecData {
                    codec_type,
                    width: None,
                    height: None,
                    extra_data: None,
                }));
                self.pids_to_stream_indices.push((info.elementary_pid, idx));
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
        Ok(self
            .streams
            .iter()
            .map(|s| {
                // Create a new TSCodecData from the Arc'd one
                let codec = TSCodecData {
                    codec_type: s.codec_type,
                    width: s.width,
                    height: s.height,
                    extra_data: s.extra_data.clone(),
                };
                // Box it as a CodecData trait object
                Box::new(codec) as Box<dyn av::CodecData>
            })
            .collect())
    }
}
