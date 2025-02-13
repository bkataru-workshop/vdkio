use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodecType {
    H264,
    H265, // Added H265 codec type
    AAC,
    OPUS,
    // ... other codecs
}

#[async_trait]
pub trait CodecData: Send + Sync {
    fn codec_type(&self) -> CodecType;
    fn width(&self) -> Option<u32>;
    fn height(&self) -> Option<u32>;
    fn extra_data(&self) -> Option<&[u8]>;
}

#[async_trait]
pub trait Demuxer: Send {
    async fn read_packet(&mut self) -> crate::Result<Packet>;
    async fn streams(&mut self) -> crate::Result<Vec<Box<dyn CodecData>>>;
}

#[async_trait]
pub trait Muxer: Send {
    async fn write_header(&mut self, streams: &[Box<dyn CodecData>]) -> crate::Result<()>;
    async fn write_packet(&mut self, packet: Packet) -> crate::Result<()>;
    async fn write_trailer(&mut self) -> crate::Result<()>;
}

mod packet;
pub use packet::*;
