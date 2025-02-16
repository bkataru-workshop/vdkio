use crate::av::{CodecDataExt, Demuxer as AvDemuxer, Muxer as AvMuxer, Packet};
use crate::codec::aac::{AACConfig, AACParser, ADTSHeader};
use crate::{Result, VdkError};
use async_trait::async_trait;
use std::io;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter};

/// AAC format muxer for writing AAC files with ADTS headers
pub struct AACMuxer<W: AsyncWrite + Unpin + Send> {
    writer: BufWriter<W>,
    config: Option<AACConfig>,
}

impl<W: AsyncWrite + Unpin + Send> AACMuxer<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: BufWriter::new(writer),
            config: None,
        }
    }
}

#[async_trait]
impl<W: AsyncWrite + Unpin + Send> AvMuxer for AACMuxer<W> {
    async fn write_header(&mut self, streams: &[Box<dyn CodecDataExt>]) -> crate::Result<()> {
        if streams.len() != 1 {
            return Err(VdkError::InvalidData(
                "AAC muxer requires exactly one stream".to_string(),
            ));
        }
        // Configuration will be extracted from first packet's ADTS header
        Ok(())
    }

    async fn write_packet(&mut self, packet: Packet) -> crate::Result<()> {
        let mut parser = AACParser::new();

        if self.config.is_none() {
            // Try to extract config from ADTS header
            if let Ok(header) = parser.parse_adts_header(&packet.data[..7]) {
                self.config = Some(AACConfig {
                    profile: header.profile,
                    sample_rate_index: header.sample_rate_index,
                    channel_configuration: header.channel_configuration,
                    frame_length: 1024, // AAC default frame length
                });
            } else {
                return Err(VdkError::InvalidData(
                    "No AAC configuration available".to_string(),
                ));
            }
        }

        // Write ADTS header
        if let Some(config) = &self.config {
            let adts_header = ADTSHeader {
                sync_word: 0xFFF,
                id: 0, // MPEG-4
                layer: 0,
                protection_absent: true,
                profile: config.profile,
                sample_rate_index: config.sample_rate_index,
                private_bit: false,
                channel_configuration: config.channel_configuration,
                original_copy: false,
                home: false,
                copyright_id_bit: false,
                copyright_id_start: false,
                frame_length: (packet.data.len() + 7) as u16, // Include ADTS header length
                buffer_fullness: 0x7FF,                       // Variable bit rate
                number_of_raw_blocks: 0,
            };

            let header_bytes = adts_header.to_bytes()?;
            self.writer.write_all(&header_bytes).await?;
        }

        // Write AAC frame data
        self.writer.write_all(&packet.data).await?;

        Ok(())
    }

    async fn write_trailer(&mut self) -> crate::Result<()> {
        self.writer.flush().await?;
        Ok(())
    }
}

/// AAC format demuxer for reading AAC files with ADTS headers
pub struct AACDemuxer<R: AsyncRead + Unpin + Send> {
    reader: R,
    parser: AACParser,
    config: Option<AACConfig>,
    current_pts: i64,
}

impl<R: AsyncRead + Unpin + Send> AACDemuxer<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            parser: AACParser::new(),
            config: None,
            current_pts: 0,
        }
    }
}

#[async_trait]
impl<R: AsyncRead + Unpin + Send> AvDemuxer for AACDemuxer<R> {
    async fn streams(&mut self) -> Result<Vec<Box<dyn CodecDataExt>>> {
        if self.config.is_none() {
            let mut header_buf = [0u8; 7]; // ADTS header size
            self.reader.read_exact(&mut header_buf).await?;

            if let Ok(header) = self.parser.parse_adts_header(&header_buf) {
                let config = AACConfig {
                    profile: header.profile,
                    sample_rate_index: header.sample_rate_index,
                    channel_configuration: header.channel_configuration,
                    frame_length: 1024, // AAC default frame length
                };
                self.config = Some(config.clone());
            }
        }

        // TODO: Create proper AAC CodecData from config
        Ok(vec![])
    }

    async fn read_packet(&mut self) -> Result<Packet> {
        let mut header_buf = [0u8; 7];
        match self.reader.read_exact(&mut header_buf).await {
            Ok(_) => {
                if let Ok(header) = self.parser.parse_adts_header(&header_buf) {
                    let frame_length = header.frame_length as usize;
                    if frame_length < 7 {
                        return Err(VdkError::InvalidData(
                            "Invalid frame length in ADTS header".to_string(),
                        ));
                    }
                    let mut frame_data = vec![0u8; frame_length - 7];
                    self.reader.read_exact(&mut frame_data).await?;

                    // Update timestamp based on sample rate
                    let sample_rate = match header.sample_rate_index {
                        0 => 96000,
                        1 => 88200,
                        2 => 64000,
                        3 => 48000,
                        4 => 44100, // Default to 44.1kHz
                        5 => 32000,
                        6 => 24000,
                        7 => 22050,
                        8 => 16000,
                        9 => 12000,
                        10 => 11025,
                        11 => 8000,
                        _ => 44100, // Default to 44.1kHz
                    };

                    // Calculate duration for 1024 samples
                    let duration =
                        Duration::from_nanos((1_024_000_000_000u64) / sample_rate as u64);
                    self.current_pts += duration.as_nanos() as i64;

                    Ok(Packet::new(frame_data)
                        .with_pts(self.current_pts)
                        .with_duration(duration)
                        .with_key_flag(true)
                        .with_stream_index(0))
                } else {
                    Err(VdkError::InvalidData("Invalid ADTS header".to_string()))
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                Err(VdkError::Protocol("End of stream".to_string()))
            }
            Err(e) => Err(VdkError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufWriter;
    use crate::av::CodecData;

    #[tokio::test]
    async fn test_aac_muxer() {
        let buf = Vec::new();
        let writer = BufWriter::new(buf);
        let mut muxer = AACMuxer::new(writer);

        // Create test codec data
        #[derive(Clone)]
        struct TestAACCodec;
        impl CodecData for TestAACCodec {
            fn codec_type(&self) -> crate::av::CodecType {
                crate::av::CodecType::AAC
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

        // Write header with one AAC stream
        let streams = vec![
            // TestAACCodec implements Clone and CodecData, so it gets CodecDataExt
            // through the blanket implementation
            Box::new(TestAACCodec) as Box<dyn CodecDataExt>,
        ];
        muxer.write_header(&streams).await.unwrap();

        // Create a test packet with ADTS header
        let adts_header = ADTSHeader {
            sync_word: 0xFFF,
            id: 0,
            layer: 0,
            protection_absent: true,
            profile: 1.into(),    // AAC-LC
            sample_rate_index: 4, // 44.1kHz
            private_bit: false,
            channel_configuration: 2, // Stereo
            original_copy: false,
            home: false,
            copyright_id_bit: false,
            copyright_id_start: false,
            frame_length: 1031, // 1024 + 7 (ADTS header)
            buffer_fullness: 0x7FF,
            number_of_raw_blocks: 0,
        };

        let header_bytes = adts_header.to_bytes().unwrap();
        let mut data = header_bytes.to_vec();
        data.extend_from_slice(&[0u8; 1024]); // Dummy frame data

        let packet = Packet::new(data)
            .with_pts(0)
            .with_duration(Duration::from_millis(23)) // ~1024 samples at 44.1kHz
            .with_key_flag(true);

        muxer.write_packet(packet).await.unwrap();
        muxer.write_trailer().await.unwrap();
    }
}
