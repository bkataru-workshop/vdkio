//! # RTSP Protocol Implementation
//!
//! This module provides a complete implementation of the RTSP (Real Time Streaming Protocol)
//! client, supporting features such as:
//!
//! - Connection establishment and authentication
//! - Session management
//! - Media setup and control
//! - Stream statistics and monitoring
//! - SDP (Session Description Protocol) parsing
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use vdkio::format::rtsp::{RTSPClient, RTSPSetupOptions};
//! use tokio;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client with custom options
//!     let options = RTSPSetupOptions::default()
//!         .with_authentication("username", "password")
//!         .with_timeout(std::time::Duration::from_secs(10));
//!     
//!     let client = RTSPClient::connect_with_options(
//!         "rtsp://example.com/stream",
//!         options
//!     ).await?;
//!     
//!     // Setup and start playing
//!     client.setup().await?;
//!     client.play().await?;
//!     
//!     // Read media packets
//!     while let Some(packet) = client.read_packet().await? {
//!         println!("Received packet: {:?}", packet);
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Stream Statistics
//!
//! ```rust,no_run
//! use vdkio::format::rtsp::{RTSPClient, StreamStatistics};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = RTSPClient::connect("rtsp://example.com/stream").await?;
//! 
//! // Get stream statistics
//! let stats: StreamStatistics = client.get_statistics().await?;
//! println!("Packets received: {}", stats.packets_received);
//! println!("Packets lost: {}", stats.packets_lost);
//! println!("Jitter: {} ms", stats.jitter_ms);
//! # Ok(())
//! # }
//! ```

mod client;
mod connection;
mod stream;
mod transport;

pub use client::{RTSPClient, RTSPSetupOptions};
pub use stream::{MediaStream, StreamStatistics};
pub use transport::{CastType, TransportInfo};

use thiserror::Error;

/// Errors that can occur during RTSP operations
#[derive(Debug, Error)]
pub enum RTSPError {
    /// Protocol-level errors (malformed messages, invalid sequence)
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Authentication failures
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Transport-related errors (network issues, timeout)
    #[error("Transport error: {0}")]
    Transport(String),

    /// SDP parsing errors
    #[error("Invalid SDP: {0}")]
    SDPError(String),
}

/// Helper function to parse standard SDP media descriptions and their attributes.
/// 
/// This function parses media descriptions following RFC 4566 format:
/// ```text
/// m=<media> <port> <proto> <fmt> ...
/// a=<attribute>
/// a=<attribute>:<value>
/// ```
///
/// # Arguments
///
/// * `media` - The media description string to parse
///
/// # Returns
///
/// A `Result` containing either a `MediaDescription` or an `RTSPError`
///
/// # Examples
///
/// ```
/// # use vdkio::format::rtsp::parse_sdp_media;
/// let media = "video 0 RTP/AVP 96\na=control:trackID=0\na=rtpmap:96 H264/90000";
/// let desc = parse_sdp_media(media).unwrap();
/// assert_eq!(desc.media_type, "video");
/// assert_eq!(desc.get_attribute("rtpmap").unwrap(), "96 H264/90000");
/// ```
pub(crate) fn parse_sdp_media(media: &str) -> Result<MediaDescription, RTSPError> {
    let mut lines = media.lines();
    let media_line = lines
        .next()
        .ok_or_else(|| RTSPError::SDPError("Empty media description".into()))?;

    let parts: Vec<&str> = media_line.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(RTSPError::SDPError("Invalid media description".into()));
    }

    let mut description = MediaDescription {
        media_type: parts[0].to_string(),
        port: parts[1]
            .parse()
            .map_err(|_| RTSPError::SDPError("Invalid port".into()))?,
        protocol: parts[2].to_string(),
        format: parts[3].to_string(),
        attributes: std::collections::HashMap::new(),
    };

    // Parse additional attributes
    for line in lines {
        if line.starts_with("a=") {
            if let Some(attr_str) = line.strip_prefix("a=") {
                if let Some((name, value)) = attr_str.split_once(':') {
                    description.set_attribute(name, value);
                } else {
                    description.set_attribute(attr_str, ""); // Flag attribute without value
                }
            }
        }
    }

    Ok(description)
}

/// Represents a media description in an SDP message
#[derive(Debug, Clone)]
pub struct MediaDescription {
    /// Type of media (e.g., "video", "audio")
    pub media_type: String,
    /// Port number for the media stream
    pub port: u16,
    /// Transport protocol (e.g., "RTP/AVP")
    pub protocol: String,
    /// Format identifier (e.g., payload type for RTP)
    pub format: String,
    /// Additional attributes for the media description
    pub attributes: std::collections::HashMap<String, String>,
}

impl MediaDescription {
    /// Get the value of a media attribute
    pub fn get_attribute(&self, name: &str) -> Option<&String> {
        self.attributes.get(name)
    }

    /// Set a media attribute
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        self.attributes.insert(name.to_string(), value.to_string());
    }

    /// Remove a media attribute
    pub fn remove_attribute(&mut self, name: &str) -> Option<String> {
        self.attributes.remove(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdp_media_parse() {
        let media = "video 0 RTP/AVP 96\na=control:trackID=0\na=rtpmap:96 H264/90000";
        let desc = parse_sdp_media(media).unwrap();
        assert_eq!(desc.media_type, "video");
        assert_eq!(desc.port, 0);
        assert_eq!(desc.protocol, "RTP/AVP");
        assert_eq!(desc.format, "96");
        assert_eq!(desc.get_attribute("control").unwrap(), "trackID=0");
        assert_eq!(desc.get_attribute("rtpmap").unwrap(), "96 H264/90000");
    }

    #[test]
    fn test_media_description_attributes() {
        let mut desc = MediaDescription {
            media_type: "video".to_string(),
            port: 0,
            protocol: "RTP/AVP".to_string(),
            format: "96".to_string(),
            attributes: std::collections::HashMap::new(),
        };

        // Set and get attribute
        desc.set_attribute("control", "trackID=0");
        assert_eq!(desc.get_attribute("control").unwrap(), "trackID=0");

        // Update existing attribute
        desc.set_attribute("control", "trackID=1");
        assert_eq!(desc.get_attribute("control").unwrap(), "trackID=1");

        // Remove attribute
        assert_eq!(desc.remove_attribute("control").unwrap(), "trackID=1");
        assert!(desc.get_attribute("control").is_none());
    }
}
