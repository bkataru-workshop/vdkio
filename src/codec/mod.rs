//! # Video and Audio Codec Implementations
//!
//! This module provides implementations for various video and audio codecs,
//! focusing on parsing, encoding, and decoding functionality. The implementations
//! are designed to be efficient and suitable for real-time streaming applications.
//!
//! ## Supported Codecs
//!
//! ### H.264/AVC
//! Complete implementation with support for:
//! - NAL unit parsing and extraction
//! - Parameter sets (SPS/PPS) handling
//! - Frame type detection
//! - Resolution changes
//! - Transcoding capabilities
//!
//! ```rust,no_run
//! use vdkio::codec::h264::{H264Decoder, H264Encoder};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let decoder = H264Decoder::new();
//! let encoder = H264Encoder::new_with_resolution(1920, 1080)?;
//!
//! // Configure encoder parameters
//! encoder.set_bitrate(5_000_000)?; // 5 Mbps
//! encoder.set_framerate(30)?;
//! # Ok(())
//! # }
//! ```
//!
//! ### H.265/HEVC
//! Basic implementation supporting:
//! - NAL unit parsing
//! - Parameter sets handling
//! - Frame extraction
//!
//! ### AAC Audio
//! Implementation supporting:
//! - ADTS frame parsing
//! - Audio frame extraction
//! - Basic stream configuration
//!
//! ## Transcoding Support
//!
//! The codec module provides transcoding capabilities, particularly for H.264:
//!
//! ```rust,no_run
//! use vdkio::codec::create_transcoder_for_resolution;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a transcoder for 720p output
//! let transcoder = create_transcoder_for_resolution(1280, 720)?;
//!
//! // Configure transcoding parameters
//! transcoder.set_target_bitrate(2_000_000)?; // 2 Mbps
//! transcoder.set_keyframe_interval(60)?; // keyframe every 2 seconds at 30fps
//! # Ok(())
//! # }
//! ```

#[doc = "AAC (Advanced Audio Coding) codec implementation

Provides ADTS frame parsing and audio extraction capabilities"]
pub mod aac;

#[doc = "H.264/AVC (Advanced Video Coding) codec implementation

Complete implementation supporting NAL parsing, parameter sets,
frame extraction, and transcoding capabilities"]
pub mod h264;

#[doc = "H.265/HEVC (High Efficiency Video Coding) codec implementation

Basic implementation supporting NAL parsing, parameter sets handling,
and frame extraction"]
pub mod h265;

// Re-export common types and functions
#[doc(inline)]
pub use h264::parser::NALUnit;
#[doc(inline)]
pub use h264::transcode::create_transcoder_for_resolution;
#[doc(inline)]
pub use h264::transcode::H264Decoder;
#[doc(inline)]
pub use h264::transcode::H264Encoder;
