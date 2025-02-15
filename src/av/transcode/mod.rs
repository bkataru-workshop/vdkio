use std::time::Duration;
use crate::av::{CodecData, CodecType, Packet};
use crate::error::Result;
use async_trait::async_trait;
use std::sync::Arc;
use bytes::Bytes;

#[derive(Clone)]
pub struct VideoFrame {
    pub data: Vec<Vec<u8>>, // YUV planes
    pub width: u32,
    pub height: u32,
    pub pts: i64,
    pub key_frame: bool,
}

#[derive(Clone)]
pub struct AudioFrame {
    pub data: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u8,
    pub pts: i64,
}

#[derive(Clone)]
pub struct StreamCodecData {
    pub codec_type: CodecType,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub extra_data: Option<Vec<u8>>,
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

#[async_trait]
pub trait VideoEncoder: Send + Sync {
    fn codec_data(&self) -> StreamCodecData;
    async fn encode(&mut self, frame: VideoFrame) -> Result<Vec<Vec<u8>>>;
    fn close(&mut self);
}

#[async_trait]
pub trait VideoDecoder: Send + Sync {
    async fn decode(&mut self, data: Vec<u8>) -> Result<Option<VideoFrame>>;
    fn close(&mut self);
}

#[async_trait]
pub trait AudioEncoder: Send + Sync {
    fn codec_data(&self) -> StreamCodecData;
    async fn encode(&mut self, frame: AudioFrame) -> Result<Vec<Vec<u8>>>;
    fn close(&mut self);
}

#[async_trait]
pub trait AudioDecoder: Send + Sync {
    async fn decode(&mut self, data: Vec<u8>) -> Result<Option<AudioFrame>>;
    fn close(&mut self);
}

struct Stream {
    codec: StreamCodecData,
    timeline: Timeline,
    video_encoder: Option<Box<dyn VideoEncoder>>,
    video_decoder: Option<Box<dyn VideoDecoder>>,
    audio_encoder: Option<Box<dyn AudioEncoder>>,
    audio_decoder: Option<Box<dyn AudioDecoder>>,
}

#[derive(Clone)]
struct Timeline {
    time: i64,
    duration: Duration,
}

impl Timeline {
    fn new() -> Self {
        Self {
            time: 0,
            duration: Duration::from_secs(0),
        }
    }

    fn push(&mut self, time: i64, duration: Duration) {
        self.time = time;
        self.duration = duration;
    }

    fn pop(&mut self, duration: Duration) -> i64 {
        let time = self.time;
        self.time += duration.as_nanos() as i64;
        time
    }
}

pub struct TranscodeOptions {
    pub find_video_codec: Option<Box<dyn Fn(&StreamCodecData) -> Result<(Box<dyn VideoEncoder>, Box<dyn VideoDecoder>)> + Send + Sync>>,
    pub find_audio_codec: Option<Box<dyn Fn(&StreamCodecData) -> Result<(Box<dyn AudioEncoder>, Box<dyn AudioDecoder>)> + Send + Sync>>,
}

pub struct Transcoder {
    streams: Vec<Stream>,
    options: Arc<TranscodeOptions>,
}

impl Transcoder {
    pub fn new(codecs: Vec<StreamCodecData>, options: TranscodeOptions) -> Result<Self> {
        let mut out_streams = Vec::new();
        let options = Arc::new(options);

        for codec in codecs {
            let mut s = Stream {
                codec: codec.clone(),
                timeline: Timeline::new(),
                video_encoder: None,
                video_decoder: None,
                audio_encoder: None,
                audio_decoder: None,
            };

            match codec.codec_type() {
                CodecType::H264 | CodecType::H265 => {
                    if let Some(ref find_codec) = options.find_video_codec {
                        let (enc, dec) = find_codec(&codec)?;
                        s.video_encoder = Some(enc);
                        s.video_decoder = Some(dec);
                    }
                }
                CodecType::AAC | CodecType::OPUS => {
                    if let Some(ref find_codec) = options.find_audio_codec {
                        let (enc, dec) = find_codec(&codec)?;
                        s.audio_encoder = Some(enc);
                        s.audio_decoder = Some(dec);
                    }
                }
                codec_type => {
                    return Err(VdkError::Codec(format!("Unsupported codec type for transcoding: {:?}", codec_type)));
                }
            }

            out_streams.push(s);
        }

        Ok(Self {
            streams: out_streams,
            options,
        })
    }

    pub async fn transcode_packet(&mut self, pkt: Packet) -> Result<Vec<Packet>> {
        let stream = &mut self.streams[pkt.stream_index];
        let mut out_packets = Vec::new();

        if let Some(ref mut dec) = stream.video_decoder {
            if let Some(ref mut enc) = stream.video_encoder {
                if let Some(frame) = dec.decode(pkt.data.to_vec()).await? {
                    for encoded in enc.encode(frame).await? {
                        let packet = Packet::new(Bytes::copy_from_slice(&encoded))
                            .with_pts(stream.timeline.pop(pkt.duration.unwrap_or_default()))
                            .with_stream_index(pkt.stream_index);
                        out_packets.push(packet);
                    }
                }
            }
        } else if let Some(ref mut dec) = stream.audio_decoder {
            if let Some(ref mut enc) = stream.audio_encoder {
                if let Some(frame) = dec.decode(pkt.data.to_vec()).await? {
                    for encoded in enc.encode(frame).await? {
                        let packet = Packet::new(Bytes::copy_from_slice(&encoded))
                            .with_pts(stream.timeline.pop(pkt.duration.unwrap_or_default()))
                            .with_stream_index(pkt.stream_index);
                        out_packets.push(packet);
                    }
                }
            }
        } else {
            out_packets.push(pkt);
        }

        Ok(out_packets)
    }

    pub fn streams(&self) -> Vec<StreamCodecData> {
        self.streams.iter()
            .map(|s| s.codec.clone())
            .collect()
    }

    pub fn close(&mut self) {
        for stream in &mut self.streams {
            if let Some(ref mut enc) = stream.video_encoder {
                enc.close();
            }
            if let Some(ref mut dec) = stream.video_decoder {
                dec.close();
            }
            if let Some(ref mut enc) = stream.audio_encoder {
                enc.close();
            }
            if let Some(ref mut dec) = stream.audio_decoder {
                dec.close();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transcoder_passthrough() {
        let codecs = vec![
            StreamCodecData {
                codec_type: CodecType::H264,
                width: Some(1920),
                height: Some(1080),
                extra_data: None,
            }
        ];

        let options = TranscodeOptions {
            find_video_codec: None,
            find_audio_codec: None,
        };

        let mut transcoder = Transcoder::new(codecs, options).unwrap();

        let input_packet = Packet::new(Bytes::from(vec![1, 2, 3]))
            .with_stream_index(0)
            .with_pts(1000);

        let output_packets = transcoder.transcode_packet(input_packet.clone()).await.unwrap();
        assert_eq!(output_packets.len(), 1);
        assert_eq!(output_packets[0].stream_index, input_packet.stream_index);
        assert_eq!(output_packets[0].pts, input_packet.pts);
    }
}