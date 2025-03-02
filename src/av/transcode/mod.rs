use crate::av::{CodecData, CodecType, Packet};
use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;

/// Represents a decoded video frame in YUV format.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// The YUV plane data. Index 0 is Y, 1 is U, 2 is V
    pub data: Vec<Vec<u8>>,
    /// Width of the video frame in pixels
    pub width: u32,
    /// Height of the video frame in pixels
    pub height: u32,
    /// Presentation timestamp of the frame
    pub pts: i64,
    /// Indicates if this is a key frame
    pub key_frame: bool,
}

/// Represents a decoded audio frame.
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// Raw audio sample data
    pub data: Vec<u8>,
    /// Sample rate in Hz (e.g., 44100, 48000)
    pub sample_rate: u32,
    /// Number of audio channels
    pub channels: u8,
    /// Presentation timestamp of the frame
    pub pts: i64,
}

/// Contains codec-specific information for a media stream.
#[derive(Debug, Clone)]
pub struct StreamCodecData {
    /// Type of codec (H264, AAC, etc.)
    pub codec_type: CodecType,
    /// Width in pixels for video streams
    pub width: Option<u32>,
    /// Height in pixels for video streams
    pub height: Option<u32>,
    /// Codec-specific extra data (e.g., SPS/PPS for H.264)
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

/// Interface for video encoders.
#[async_trait]
pub trait VideoEncoder: Send + Sync {
    /// Returns the codec configuration for this encoder
    fn codec_data(&self) -> StreamCodecData;
    /// Encodes a video frame into one or more compressed packets
    async fn encode(&mut self, frame: VideoFrame) -> Result<Vec<Vec<u8>>>;
    /// Releases any resources held by the encoder
    fn close(&mut self);
}

/// Interface for video decoders.
#[async_trait]
pub trait VideoDecoder: Send + Sync {
    /// Decodes compressed video data into a frame
    async fn decode(&mut self, data: Vec<u8>) -> Result<Option<VideoFrame>>;
    /// Releases any resources held by the decoder
    fn close(&mut self);
}

/// Interface for audio encoders.
#[async_trait]
pub trait AudioEncoder: Send + Sync {
    /// Returns the codec configuration for this encoder
    fn codec_data(&self) -> StreamCodecData;
    /// Encodes an audio frame into one or more compressed packets
    async fn encode(&mut self, frame: AudioFrame) -> Result<Vec<Vec<u8>>>;
    /// Releases any resources held by the encoder
    fn close(&mut self);
}

/// Interface for audio decoders.
#[async_trait]
pub trait AudioDecoder: Send + Sync {
    /// Decodes compressed audio data into a frame
    async fn decode(&mut self, data: Vec<u8>) -> Result<Option<AudioFrame>>;
    /// Releases any resources held by the decoder
    fn close(&mut self);
}

/// Internal representation of a media stream with its codecs.
struct Stream {
    codec: StreamCodecData,
    timeline: Timeline,
    video_encoder: Option<Box<dyn VideoEncoder>>,
    video_decoder: Option<Box<dyn VideoDecoder>>,
    audio_encoder: Option<Box<dyn AudioEncoder>>,
    audio_decoder: Option<Box<dyn AudioDecoder>>,
}

// Custom Debug implementation for Stream
impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("codec", &self.codec)
            .field("timeline", &self.timeline)
            .field("video_encoder", &self.video_encoder.is_some())
            .field("video_decoder", &self.video_decoder.is_some())
            .field("audio_encoder", &self.audio_encoder.is_some())
            .field("audio_decoder", &self.audio_decoder.is_some())
            .finish()
    }
}

/// Manages timing information for media streams.
#[derive(Debug, Clone)]
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

    #[allow(dead_code)]
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

/// Configuration options for the transcoder.
pub struct TranscodeOptions {
    /// Function to create video codec pairs for a given stream
    pub find_video_codec: Option<
        Box<
            dyn Fn(&StreamCodecData) -> Result<(Box<dyn VideoEncoder>, Box<dyn VideoDecoder>)>
                + Send
                + Sync,
        >,
    >,
    /// Function to create audio codec pairs for a given stream
    pub find_audio_codec: Option<
        Box<
            dyn Fn(&StreamCodecData) -> Result<(Box<dyn AudioEncoder>, Box<dyn AudioDecoder>)>
                + Send
                + Sync,
        >,
    >,
}

impl std::fmt::Debug for TranscodeOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranscodeOptions")
            .field("has_video_codec", &self.find_video_codec.is_some())
            .field("has_audio_codec", &self.find_audio_codec.is_some())
            .finish()
    }
}

/// Main transcoding engine that manages codec conversions for multiple streams.
pub struct Transcoder {
    /// List of media streams being processed
    streams: Vec<Stream>,
    /// Transcoding configuration options
    options: Arc<TranscodeOptions>,
}

impl std::fmt::Debug for Transcoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transcoder")
            .field("streams", &self.streams)
            .field("options", &self.options)
            .finish()
    }
}

impl Transcoder {
    /// Creates a new transcoder instance.
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
            }

            out_streams.push(s);
        }

        Ok(Self {
            streams: out_streams,
            options,
        })
    }

    /// Transcodes a single media packet.
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

    /// Returns the codec configurations for all streams.
    pub fn streams(&self) -> Vec<StreamCodecData> {
        self.streams.iter().map(|s| s.codec.clone()).collect()
    }

    /// Releases resources held by all codecs.
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
        let codecs = vec![StreamCodecData {
            codec_type: CodecType::H264,
            width: Some(1920),
            height: Some(1080),
            extra_data: None,
        }];

        let options = TranscodeOptions {
            find_video_codec: None,
            find_audio_codec: None,
        };

        let mut transcoder = Transcoder::new(codecs, options).unwrap();

        let input_packet = Packet::new(Bytes::from(vec![1, 2, 3]))
            .with_stream_index(0)
            .with_pts(1000);

        let output_packets = transcoder
            .transcode_packet(input_packet.clone())
            .await
            .unwrap();
        assert_eq!(output_packets.len(), 1);
        assert_eq!(output_packets[0].stream_index, input_packet.stream_index);
        assert_eq!(output_packets[0].pts, input_packet.pts);
    }
}
