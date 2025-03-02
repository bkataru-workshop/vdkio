//! # H.265/HEVC Codec Implementation
//!
//! This module provides functionality for parsing and handling H.265/HEVC (High Efficiency Video Coding)
//! video streams. The implementation supports basic H.265 features including:
//!
//! - NAL unit parsing and extraction
//! - Parameter sets handling (VPS, SPS, PPS)
//! - Frame type detection and handling
//! - Basic stream configuration
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use vdkio::codec::h265::H265Parser;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut parser = H265Parser::new();
//!
//! // Parse H.265 NAL units from raw data
//! let raw_data = vec![0u8; 1024]; // Example raw H.265 data
//! let nal_units = parser.parse_nal_units(&raw_data)?;
//!
//! for nal in nal_units {
//!     println!("Found NAL unit type: {:?}", nal.nal_type());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Current Implementation Status
//!
//! The H.265 implementation currently provides basic functionality and is being
//! actively developed. While core features are implemented and tested, some advanced
//! features are still in progress.
//!
//! ### Implemented Features
//! - Basic NAL unit parsing
//! - Parameter sets extraction
//! - Frame boundary detection
//! - Stream configuration parsing
//!
//! ### In Progress
//! - Advanced parameter set handling
//! - Complete support for all NAL unit types
//! - Performance optimizations
//! - Comprehensive validation with varied streams

/// Parser implementation for H.265/HEVC streams
pub mod parser;

/// Type definitions and structures for H.265/HEVC codec
pub mod types;

// Re-export commonly used types for convenience
pub use parser::H265Parser;
