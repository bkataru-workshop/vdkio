use crate::av::{Packet, CodecData};
use crate::Result;

pub mod rtsp;

/// Common trait for format demuxers
#[async_trait::async_trait]
pub trait Demuxer: Send {
    /// Read the next packet from the stream
    async fn read_packet(&mut self) -> Result<Packet>;
    
    /// Get stream information
    async fn streams(&mut self) -> Result<Vec<Box<dyn CodecData>>>;
}

/// Common trait for format muxers
#[async_trait::async_trait]
pub trait Muxer: Send {
    /// Write stream header information
    async fn write_header(&mut self, streams: &[Box<dyn CodecData>]) -> Result<()>;
    
    /// Write a packet to the stream
    async fn write_packet(&mut self, packet: &Packet) -> Result<()>;
    
    /// Write stream trailer information
    async fn write_trailer(&mut self) -> Result<()>;
    
    /// Flush any buffered packets
    async fn flush(&mut self) -> Result<()>;
}
