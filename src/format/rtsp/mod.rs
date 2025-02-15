mod client;
mod connection;
mod transport;
mod stream;

pub use client::{RTSPClient, RTSPSetupOptions};
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

/// Helper function to parse standard SDP media descriptions and their attributes
pub(crate) fn parse_sdp_media(media: &str) -> Result<MediaDescription, RTSPError> {
    let mut lines = media.lines();
    let media_line = lines.next()
        .ok_or_else(|| RTSPError::SDPError("Empty media description".into()))?;

    let parts: Vec<&str> = media_line.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(RTSPError::SDPError("Invalid media description".into()));
    }

    let mut description = MediaDescription {
        media_type: parts[0].to_string(),
        port: parts[1].parse().map_err(|_| RTSPError::SDPError("Invalid port".into()))?,
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

    pub fn set_attribute(&mut self, name: &str, value: &str) {
        self.attributes.insert(name.to_string(), value.to_string());
    }

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
