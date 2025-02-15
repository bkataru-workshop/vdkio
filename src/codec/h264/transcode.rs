use crate::av::transcode::{VideoFrame, VideoEncoder, VideoDecoder, StreamCodecData};
use crate::av::CodecType;
use crate::error::{Result, VdkError};
use async_trait::async_trait;
use super::parser::{NALUnit, NALType};

#[allow(dead_code)]
pub struct H264Decoder {
    width: u32,
    height: u32,
    extra_data: Option<Vec<u8>>,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

impl H264Decoder {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            extra_data: None,
            sps: None,
            pps: None,
        }
    }
}

#[async_trait]
impl VideoDecoder for H264Decoder {
    async fn decode(&mut self, data: Vec<u8>) -> Result<Option<VideoFrame>> {
        // Parse H.264 NAL units
        let nals = NALUnit::find_units(&data)?;
        
        // Process each NAL unit
        for nal in nals {
            match nal.nal_type()? {
                NALType::SPS => {
                    self.sps = Some(nal.payload().to_vec());
                }
                NALType::PPS => {
                    self.pps = Some(nal.payload().to_vec());
                }
                NALType::IDR | NALType::NonIDR => {
                    // For now, just pass through the data
                    // TODO: Actually decode H.264 frames to YUV
                    return Ok(Some(VideoFrame {
                        data: vec![data.clone()], // Single plane for now
                        width: self.width,
                        height: self.height,
                        pts: 0, // TODO: Extract PTS
                        key_frame: nal.nal_type()? == NALType::IDR,
                    }));
                }
                _ => continue,
            }
        }
        
        Ok(None)
    }

    fn close(&mut self) {
        // Clean up resources
    }
}

#[allow(dead_code)]
pub struct H264Encoder {
    width: u32,
    height: u32,
    bitrate: u32,
    fps: u32,
    extra_data: Option<Vec<u8>>,
}

impl H264Encoder {
    pub fn new(width: u32, height: u32, bitrate: u32, fps: u32) -> Self {
        Self {
            width,
            height,
            bitrate,
            fps,
            extra_data: None,
        }
    }
}

#[async_trait]
impl VideoEncoder for H264Encoder {
    fn codec_data(&self) -> StreamCodecData {
        StreamCodecData {
            codec_type: CodecType::H264,
            width: Some(self.width),
            height: Some(self.height),
            extra_data: self.extra_data.clone(),
        }
    }

    async fn encode(&mut self, frame: VideoFrame) -> Result<Vec<Vec<u8>>> {
        // TODO: Implement actual H.264 encoding
        // For now, just pass through the data with proper NAL framing
        let mut output = Vec::new();
        
        // Add SPS/PPS for keyframes
        if frame.key_frame {
            if let Some(ref extra_data) = self.extra_data {
                output.push(extra_data.clone());
            }
        }
        
        // Pass through existing encoded data
        for plane in frame.data {
            output.push(plane);
        }
        
        Ok(output)
    }

    fn close(&mut self) {
        // Clean up resources
    }
}

pub fn create_transcoder_for_resolution(width: u32, height: u32, bitrate: u32, fps: u32) 
    -> Box<dyn Fn(&StreamCodecData) -> Result<(Box<dyn VideoEncoder>, Box<dyn VideoDecoder>)> + Send + Sync> 
{
    Box::new(move |codec: &StreamCodecData| {
        if codec.codec_type != CodecType::H264 {
            return Err(VdkError::Codec("Unsupported codec type".into()));
        }

        let decoder = Box::new(H264Decoder::new(
            codec.width.unwrap_or(1920),
            codec.height.unwrap_or(1080),
        ));

        let encoder = Box::new(H264Encoder::new(width, height, bitrate, fps));

        Ok((encoder, decoder))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_h264_passthrough() {
        let mut decoder = H264Decoder::new(1920, 1080);
        let mut encoder = H264Encoder::new(1280, 720, 2_000_000, 30);

        // Sample H.264 NAL unit (dummy data)
        let input = vec![
            0x00, 0x00, 0x00, 0x01, // Start code
            0x65, // NAL unit header (IDR)
            0x00, 0x01, 0x02, // Dummy payload
        ];

        // Decode
        let frame = decoder.decode(input.clone()).await.unwrap().unwrap();
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert!(frame.key_frame);

        // Encode at lower resolution
        let packets = encoder.encode(frame).await.unwrap();
        assert!(!packets.is_empty());
        
        // The output should contain at least the input data
        assert!(packets.iter().any(|p| p.len() >= input.len()));
    }
}