use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodecType {
    H264,
    H265,
    AAC,
    OPUS,
}

#[async_trait]
pub trait CodecData: Send + Sync {
    fn codec_type(&self) -> CodecType;
    fn width(&self) -> Option<u32>;
    fn height(&self) -> Option<u32>;
    fn extra_data(&self) -> Option<&[u8]>;
}

pub trait CodecDataExt: CodecData {
    fn box_clone(&self) -> Box<dyn CodecData>;
}

impl<T: CodecData + Clone + 'static> CodecDataExt for T {
    fn box_clone(&self) -> Box<dyn CodecData> {
        Box::new(self.clone())
    }
}

#[async_trait]
pub trait Demuxer: Send {
    async fn read_packet(&mut self) -> crate::Result<Packet>;
    async fn streams(&mut self) -> crate::Result<Vec<Box<dyn CodecDataExt>>>;
}

#[async_trait]
pub trait Muxer: Send {
    async fn write_header(&mut self, streams: &[Box<dyn CodecDataExt>]) -> crate::Result<()>;
    async fn write_packet(&mut self, packet: Packet) -> crate::Result<()>;
    async fn write_trailer(&mut self) -> crate::Result<()>;
}

pub mod packet;
pub use packet::Packet;

pub mod transcode;
pub use transcode::*;
