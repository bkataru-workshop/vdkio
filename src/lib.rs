#![doc(html_root_url = "https://docs.rs/vdkio/0.1.0")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![deny(rustdoc::missing_crate_level_docs)]

//! # vdkio - Rust Video Development Kit
//! 
//! `vdkio` is a comprehensive toolkit for building video streaming applications in Rust.
//! It provides a collection of modules for handling various video formats, codecs,
//! and streaming protocols, with a primary focus on RTSP to HLS conversion for 
//! web-based video streaming applications.
//!
//! ## Features
//!
//! ### Video Codec Support
//! - H.264/AVC parsing and frame extraction
//! - H.265/HEVC parsing and frame extraction 
//! - AAC audio parsing and frame extraction
//!
//! ### Streaming Protocols
//! - RTSP client implementation with SDP parsing
//! - RTP packet handling and media transport
//! - RTCP feedback and statistics
//! - TS (Transport Stream) format support
//! - HLS output with segmentation
//!
//! ## Quick Start
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! vdkio = "0.1.0"
//! ```
//!
//! ### RTSP Client Example
//!
//! ```rust,no_run
//! use vdkio::format::rtsp::RTSPClient;
//! use tokio;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = RTSPClient::connect("rtsp://example.com/stream").await?;
//!     
//!     // Setup video and audio streams
//!     client.setup().await?;
//!     
//!     // Start playing
//!     client.play().await?;
//!     
//!     // Process media packets
//!     while let Some(packet) = client.read_packet().await? {
//!         println!("Received packet: {:?}", packet);
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### RTSP to HLS Conversion Example
//!
//! ```rust,no_run
//! use vdkio::format::{rtsp::RTSPClient, ts::TSMuxer};
//! use tokio;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize RTSP client
//!     let rtsp = RTSPClient::connect("rtsp://example.com/stream").await?;
//!     
//!     // Create TS muxer for HLS
//!     let mut muxer = TSMuxer::new();
//!     
//!     // Configure segment duration
//!     muxer.set_segment_duration(std::time::Duration::from_secs(6));
//!     
//!     // Start the conversion pipeline
//!     while let Some(packet) = rtsp.read_packet().await? {
//!         muxer.write_packet(&packet)?;
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Module Overview
//!
//! - `av`: Core audio/video types and utilities for media handling
//!   - Packet and frame abstractions
//!   - Stream management
//!   - Transcoding support
//!
//! - `codec`: Codec implementations for various formats
//!   - H.264/AVC codec support
//!   - H.265/HEVC codec support
//!   - AAC audio codec support
//!
//! - `format`: Media container and streaming protocol implementations
//!   - RTSP client with full protocol support
//!   - RTP/RTCP packet handling
//!   - TS (Transport Stream) format
//!   - HLS segmentation and playlist generation
//!
//! - `error`: Error handling types and utilities
//!   - Custom error types for different failure scenarios
//!   - Result type alias for convenience
//!
//! - `utils`: Common utilities and helper functions
//!   - Bitstream reading/writing
//!   - CRC calculations
//!   - Buffer management
//!
/// Audio/Video base types and utilities
pub mod av;

/// Codec implementations for video and audio formats
pub mod codec;

/// Error types and utilities
pub mod error;

/// Media format implementations (RTSP, TS, HLS, etc.)
pub mod format;

/// Common utilities and helper functions
pub mod utils;

/// Configuration module
pub mod config;

pub use error::{Result, VdkError};

// Re-export transcode module for convenience
pub use av::transcode;
