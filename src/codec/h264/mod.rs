//! # H.264/AVC Codec Implementation
//!
//! This module provides a complete implementation of H.264/AVC (Advanced Video Coding) parsing
//! and transcoding functionality. It supports:
//!
//! - NAL unit parsing and handling
//! - Sequence and Picture Parameter Sets (SPS/PPS)
//! - Frame type detection and extraction
//! - Resolution and bitrate transcoding
//! - Multi-threaded encoding/decoding
//!
//! ## Example: Parsing H.264 Stream
//!
//! ```rust
//! use vdkio::codec::h264::{H264Parser, NALUnit};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut parser = H264Parser::new();
//! let data = vec![0u8; 1024]; // Example H.264 data
//!
//! let nal_units = parser.parse_nal_units(&data)?;
//! for nal in nal_units {
//!     match nal.nal_type() {
//!         5 => println!("Found IDR frame"),
//!         7 => println!("Found SPS"),
//!         8 => println!("Found PPS"),
//!         _ => println!("Found NAL type {}", nal.nal_type()),
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Example: Transcoding
//!
//! ```rust
//! use vdkio::codec::h264::{H264Decoder, H264Encoder, create_transcoder_for_resolution};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create encoder/decoder for 720p output
//! let factory = create_transcoder_for_resolution(1280, 720, 2_000_000, 30);
//!
//! // Factory creates compatible encoder/decoder pairs based on input stream properties
//! let stream_info = get_stream_info();
//! let (encoder, decoder) = factory(&stream_info)?;
//!
//! // Configure encoder
//! encoder.set_bitrate(2_000_000)?; // 2 Mbps
//! encoder.set_gop_size(30)?;        // 1 second GOP at 30fps
//! # Ok(())
//! # }
//! # fn get_stream_info() -> vdkio::av::transcode::StreamCodecData {
//! #     unimplemented!()
//! # }
//! ```

/// Parser for H.264 bitstreams, implementing NAL unit extraction and parsing
pub mod parser;
/// Transcoding functionalities for H.264, including encoding and decoding
pub mod transcode;

// Re-export commonly used types from submodules for easier access
#[doc(inline)]
pub use parser::*;
#[doc(inline)]
pub use transcode::*;
