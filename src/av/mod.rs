//! # Audio/Video Core Types and Traits
//!
//! This module provides the core types and traits for handling audio and video data
//! in the vdkio library. It defines the fundamental abstractions for:
//!
//! - Codec type identification and configuration
//! - Media packet processing
//! - Demuxing and muxing operations
//! - Transcoding capabilities
//!
//! ## Example Usage
//!
//! ```rust
//! use vdkio::av::{CodecType, CodecData, Packet, Demuxer};
//! use async_trait::async_trait;
//!
//! struct VideoStream {
//!     width: u32,
//!     height: u32,
//! }
//!
//! #[async_trait]
//! impl CodecData for VideoStream {
//!     fn codec_type(&self) -> CodecType {
//!         CodecType::H264
//!     }
//!
//!     fn width(&self) -> Option<u32> {
//!         Some(self.width)
//!     }
//!
//!     fn height(&self) -> Option<u32> {
//!         Some(self.height)
//!     }
//!
//!     fn extra_data(&self) -> Option<&[u8]> {
//!         None
//!     }
//! }
//! ```

use async_trait::async_trait;

/// Supported codec types for audio and video streams
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodecType {
    /// H.264/AVC video codec
    H264,
    /// H.265/HEVC video codec
    H265,
    /// Advanced Audio Coding (AAC)
    AAC,
    /// Opus audio codec
    OPUS,
}

/// Trait for accessing codec-specific configuration and metadata
#[async_trait]
pub trait CodecData: Send + Sync {
    /// Returns the type of codec used for this stream
    fn codec_type(&self) -> CodecType;
    
    /// Returns the width of video streams, if applicable
    fn width(&self) -> Option<u32>;
    
    /// Returns the height of video streams, if applicable
    fn height(&self) -> Option<u32>;
    
    /// Returns codec-specific extra data (e.g., SPS/PPS for H.264)
    fn extra_data(&self) -> Option<&[u8]>;
}

/// Extension trait for cloning boxed CodecData
pub trait CodecDataExt: CodecData {
    /// Creates a boxed clone of the codec data
    fn box_clone(&self) -> Box<dyn CodecData>;
}

impl<T: CodecData + Clone + 'static> CodecDataExt for T {
    fn box_clone(&self) -> Box<dyn CodecData> {
        Box::new(self.clone())
    }
}

/// Trait for demuxing (extracting) media packets from container formats
#[async_trait]
pub trait Demuxer: Send {
    /// Reads the next packet from the stream
    async fn read_packet(&mut self) -> crate::Result<Packet>;
    
    /// Returns information about all available streams
    async fn streams(&mut self) -> crate::Result<Vec<Box<dyn CodecDataExt>>>;
}

/// Trait for muxing (packaging) media packets into container formats
#[async_trait]
pub trait Muxer: Send {
    /// Writes container format header with stream information
    async fn write_header(&mut self, streams: &[Box<dyn CodecDataExt>]) -> crate::Result<()>;
    
    /// Writes a media packet to the container
    async fn write_packet(&mut self, packet: Packet) -> crate::Result<()>;
    
    /// Writes container format trailer and finalizes the output
    async fn write_trailer(&mut self) -> crate::Result<()>;
}

/// Media packet handling and management
pub mod packet;
pub use packet::Packet;

/// Transcoding functionality for converting between formats
pub mod transcode;
pub use transcode::*;
