//! # Media Format Implementations
//!
//! This module provides implementations for various media container formats and streaming protocols.
//! It includes support for:
//!
//! - **AAC**: AAC audio format handling
//! - **RTCP**: RTP Control Protocol for stream feedback
//! - **RTP**: Real-time Transport Protocol
//! - **RTSP**: Real Time Streaming Protocol
//! - **TS**: MPEG Transport Stream format
//!
//! ## Examples
//!
//! ### Using RTSP Client
//!
//! ```rust,no_run
//! use vdkio::format::rtsp::{RTSPClient, RTSPSetupOptions};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let options = RTSPSetupOptions::new()
//!     .with_video(true)
//!     .with_audio(true);
//!
//! let mut client = RTSPClient::connect_with_options(
//!     "rtsp://example.com/stream",
//!     options
//! ).await?;
//!
//! // Start playback
//! client.play().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Muxing to TS Format
//!
//! ```rust,no_run
//! use vdkio::format::ts::TSMuxer;
//! use std::fs::File;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let output = File::create("output.ts")?;
//! let mut muxer = TSMuxer::new(output);
//!
//! // Write media data
//! muxer.write_header(&[])?;
//! // muxer.write_packet(&packet)?;
//! muxer.write_trailer()?;
//! # Ok(())
//! # }
//! ```

use crate::av::{CodecData, CodecDataExt, Packet};
use crate::Result;

/// AAC audio format implementation for handling ADTS framing and streaming
pub mod aac;
/// RTP Control Protocol (RTCP) implementation for stream feedback and statistics
pub mod rtcp;
/// Real-time Transport Protocol (RTP) implementation for media streaming
pub mod rtp;
/// Real Time Streaming Protocol (RTSP) implementation with client/server support
pub mod rtsp;
/// MPEG Transport Stream (TS) format implementation with muxing/demuxing
pub mod ts;

/// Common trait for format demuxers that extract elementary streams from container formats
#[async_trait::async_trait]
pub trait Demuxer: Send {
    /// Read the next packet from the stream. Returns None when the stream is finished.
    /// 
    /// # Errors
    /// 
    /// Returns an error if:
    /// - The stream has an invalid format
    /// - There is an I/O error
    /// - The stream is corrupted
    async fn read_packet(&mut self) -> Result<Packet>;

    /// Get information about all streams in the container
    /// 
    /// # Returns
    /// 
    /// A vector of codec data descriptors, one for each elementary stream
    /// 
    /// # Errors
    /// 
    /// Returns an error if stream information cannot be retrieved
    async fn streams(&mut self) -> Result<Vec<Box<dyn CodecDataExt>>>;
}

/// Common trait for format muxers that package elementary streams into container formats
#[async_trait::async_trait]
pub trait Muxer: Send {
    /// Write container format header with stream information
    /// 
    /// # Arguments
    /// 
    /// * `streams` - Descriptors for all streams to be included
    /// 
    /// # Errors
    /// 
    /// Returns an error if the header cannot be written
    async fn write_header(&mut self, streams: &[Box<dyn CodecDataExt>]) -> Result<()>;

    /// Write a media packet to the container
    /// 
    /// # Arguments
    /// 
    /// * `packet` - The packet containing audio/video frame data
    /// 
    /// # Errors
    /// 
    /// Returns an error if the packet cannot be written
    async fn write_packet(&mut self, packet: &Packet) -> Result<()>;

    /// Write container format trailer and finalize the output
    /// 
    /// # Errors
    /// 
    /// Returns an error if the trailer cannot be written
    async fn write_trailer(&mut self) -> Result<()>;

    /// Flush any buffered packets to ensure they are written
    /// 
    /// # Errors
    /// 
    /// Returns an error if the flush operation fails
    async fn flush(&mut self) -> Result<()>;
}

/// Test utilities for format implementations
pub mod tests {
    use super::*;

    /// A test muxer implementation that collects packets for verification
    #[derive(Debug)]
    pub struct TestMuxer {
        /// Collected packets for testing
        pub packets: Vec<Packet>,
    }

    impl TestMuxer {
        /// Creates a new test muxer
        pub fn new() -> Self {
            Self {
                packets: Vec::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Muxer for TestMuxer {
        async fn write_header(&mut self, _streams: &[Box<dyn CodecDataExt>]) -> Result<()> {
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

// Re-export commonly used types
pub use self::aac::{AACDemuxer, AACMuxer};
pub use self::rtcp::{RTCPPacket, ReceptionReport};
pub use self::rtp::{JitterBuffer, RTPPacket};
pub use self::rtsp::{CastType, MediaDescription, RTSPClient, TransportInfo};
pub use self::ts::{TSDemuxer, TSMuxer};
