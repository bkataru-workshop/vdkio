use crate::av::{Packet, CodecData};
use crate::Result;

pub mod aac;
pub mod rtp;
pub mod rtcp;
pub mod rtsp;
pub mod ts;

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

pub mod tests {
    use super::*;
    
    /// A test muxer implementation
    #[derive(Debug)]
    pub struct TestMuxer {
        pub packets: Vec<Packet>,
    }

    impl TestMuxer {
        pub fn new() -> Self {
            Self {
                packets: Vec::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Muxer for TestMuxer {
        async fn write_header(&mut self, _streams: &[Box<dyn CodecData>]) -> Result<()> {
            Ok(())
        }

        async fn write_packet(&mut self, packet: &Packet) -> Result<()> {
            self.packets.push(packet.clone());
            Ok(())
        }

        async fn write_trailer(&mut self) -> Result<()> {
            Ok(())
        }

        async fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }
}

pub use self::rtp::{RTPPacket, JitterBuffer};
pub use self::rtcp::{RTCPPacket, ReceptionReport};
pub use self::rtsp::{RTSPClient, MediaDescription, TransportInfo, CastType};
pub use self::ts::{TSMuxer, TSDemuxer};
pub use self::aac::{AACMuxer, AACDemuxer};
