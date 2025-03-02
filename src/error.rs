//! # Error Types
//!
//! This module provides the error types used throughout the vdkio library.
//! It defines a central error type `VdkError` that encapsulates all possible
//! errors that can occur during video processing operations.
//!
//! ## Example Usage
//!
//! ```rust
//! use vdkio::error::{Result, VdkError};
//!
//! fn process_video_data(data: &[u8]) -> Result<()> {
//!     if data.is_empty() {
//!         return Err(VdkError::InvalidData("Empty video data".to_string()));
//!     }
//!     
//!     // Process video data...
//!     Ok(())
//! }
//! ```

use std::num::ParseIntError;
use thiserror::Error;

/// Primary error type for the vdkio library
#[derive(Error, Debug)]
pub enum VdkError {
    /// I/O errors that occur during file or network operations
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Errors related to video/audio codec operations
    #[error("codec error: {0}")]
    Codec(String),

    /// Errors related to streaming protocols (RTSP, HLS, etc.)
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Errors that occur during parsing of various formats
    #[error("parser error: {0}")]
    Parser(String),

    /// Errors for invalid or malformed input data
    #[error("invalid data: {0}")]
    InvalidData(String),

    /// Errors that occur during integer parsing
    #[error("parse int error: {0}")]
    ParseInt(#[from] ParseIntError),
}

/// A specialized Result type for vdkio operations.
///
/// This type is used throughout the vdkio library to handle operations
/// that can produce a `VdkError`.
///
/// ## Example
///
/// ```rust
/// use vdkio::error::{Result, VdkError};
///
/// fn validate_stream_id(id: &str) -> Result<i32> {
///     id.parse::<i32>().map_err(VdkError::from)
/// }
/// ```
pub type Result<T> = std::result::Result<T, VdkError>;
