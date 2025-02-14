mod client;
mod connection;
mod transport;
mod stream;

pub use client::RTSPClient;
pub use transport::{TransportInfo, CastType};
pub use stream::{MediaStream, StreamStatistics};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RTSPError {
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Invalid SDP: {0}")]
    SDPError(String),
}

/// Example usage:
/// ```no_run
/// use vdkio::format::rtsp::RTSPClient;
/// use std::error::Error;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn Error>> {
///     // Create and connect RTSP client
///     let mut client = RTSPClient::new("rtsp://example.com/stream")?;
///     client.connect().await?;
///     
///     // Get stream information via DESCRIBE
///     let media_descriptions = client.describe().await?;
///     
///     // Set up media streams from SDP
///     for media in &media_descriptions {
///         client.setup(media).await?;
///     }
///     
///     // Start playback and receive media packets
///     client.play().await?;
///     
///     if let Some(mut rx) = client.get_packet_receiver() {
///         while let Some(packet) = rx.recv().await {
///             println!("Received packet of size: {}", packet.len());
///         }
///     }
///     
///     Ok(())
/// }
/// ```

#[derive(Debug, Clone)]
pub struct MediaDescription {
    pub media_type: String,
    pub port: u16,
    pub protocol: String,
    pub format: String,
    pub attributes: std::collections::HashMap<String, String>,
}

impl MediaDescription {
    pub fn get_attribute(&self, name: &str) -> Option<&String> {
        self.attributes.get(name)
    }
}

/// Helper function to parse standard SDP media descriptions
pub(crate) fn parse_sdp_media(media: &str) -> Result<MediaDescription, RTSPError> {
    let parts: Vec<&str> = media.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(RTSPError::SDPError("Invalid media description".into()));
    }

    Ok(MediaDescription {
        media_type: parts[0].to_string(),
        port: parts[1].parse().map_err(|_| RTSPError::SDPError("Invalid port".into()))?,
        protocol: parts[2].to_string(),
        format: parts[3].to_string(),
        attributes: std::collections::HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdp_media_parse() {
        let media = "video 0 RTP/AVP 96";
        let desc = parse_sdp_media(media).unwrap();
        assert_eq!(desc.media_type, "video");
        assert_eq!(desc.port, 0);
        assert_eq!(desc.protocol, "RTP/AVP");
        assert_eq!(desc.format, "96");
    }
}
