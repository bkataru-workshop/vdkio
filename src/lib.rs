//! Rust Video Development Kit
//! 
//! A comprehensive toolkit for building video streaming applications in Rust.
//! For more information, see the [README](../README.md).

mod av;
pub mod codec;
mod error;
mod format;
mod utils;

pub use error::{Result, VdkError};

// Re-export main types
pub use av::{CodecType, CodecData, Packet};
pub use codec::{H264Parser, AACParser};
pub use codec::h265::H265Parser;
pub use format::rtsp::{RTSPClient, SessionDescription};

/// Common module containing traits and types for audio/video processing
pub mod av_mod {
    pub use crate::av::*;
}

/// Codec implementations for various audio and video formats
pub mod codecs {
    pub use crate::codec::*;
}

/// Format and protocol implementations (RTSP, etc.)
pub mod formats {
    pub use crate::format::*;
}

/// Convenient prelude module containing commonly used types
pub mod prelude {
    pub use crate::error::*;
    pub use crate::av::{CodecType, CodecData, Packet};
    pub use crate::codec::{H264Parser, AACParser};
    pub use crate::codec::h265::H265Parser;
    pub use crate::format::rtsp::{RTSPClient, SessionDescription};
    pub use crate::Result;
}
