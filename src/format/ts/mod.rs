//! # MPEG Transport Stream (TS) Implementation
//!
//! This module provides a complete implementation of MPEG Transport Stream (TS) format,
//! including support for:
//!
//! - TS packet parsing and generation
//! - Program Specific Information (PSI) tables
//! - Packetized Elementary Stream (PES) handling
//! - HLS segmentation and playlist generation
//!
//! ## Core Features
//!
//! - **Demuxing**: Extract elementary streams from TS
//! - **Muxing**: Create TS packets from elementary streams
//! - **HLS Support**: Generate HLS segments and playlists
//! - **PES Handling**: Process PES packets for video/audio
//! - **PCR Management**: Timing and synchronization
//!
//! ## Example Usage
//!
//! ### Creating a TS Muxer
//!
//! ```rust
//! use vdkio::format::ts::{TSMuxer, STREAM_TYPE_H264, TS_PACKET_SIZE};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut muxer = TSMuxer::new();
//!
//! // Configure video stream
//! muxer.add_stream(STREAM_TYPE_H264, 256)?; // PID 256 for H.264 video
//!
//! // Write TS packets
//! let mut output = Vec::new();
//! muxer.write_packets(&mut output)?;
//! assert_eq!(output.len() % TS_PACKET_SIZE, 0);
//! # Ok(())
//! # }
//! ```
//!
//! ### Using HLS Segmenter
//!
//! ```rust
//! use vdkio::format::ts::{HLSSegmenter, HLSVariant};
//! use std::time::Duration;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut segmenter = HLSSegmenter::new();
//!
//! // Configure HLS output
//! segmenter.set_segment_duration(Duration::from_secs(6));
//! segmenter.add_variant(HLSVariant::new(
//!     "high",
//!     1920,
//!     1080,
//!     5_000_000, // 5 Mbps
//! ));
//!
//! // Process incoming TS packets
//! let ts_data = vec![0u8; TS_PACKET_SIZE];
//! segmenter.process_ts_packet(&ts_data)?;
//! # Ok(())
//! # }
//! ```

/// TS demuxer implementation for extracting elementary streams
pub mod demuxer;

/// HLS-specific functionality including segmentation and playlist generation
pub mod hls;

/// TS muxer implementation for creating MPEG-TS packets
pub mod muxer;

/// Low-level TS packet parsing utilities
pub mod parser;

/// PES packet handling and management
pub mod pes;

/// Core TS types and constants
pub mod types;

// Re-export commonly used types and constants
pub use demuxer::TSDemuxer;
pub use hls::{HLSPlaylist, HLSSegment, HLSSegmenter, HLSVariant};
pub use muxer::TSMuxer;
pub use pes::{PESHeader, PESPacket};
pub use types::{
    TSHeader,
    PID_PAT,
    PID_PMT,
    STREAM_TYPE_AAC,
    STREAM_TYPE_H264,
    STREAM_TYPE_H265,
    TS_PACKET_SIZE,
};
