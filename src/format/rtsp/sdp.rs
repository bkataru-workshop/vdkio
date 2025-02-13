use std::collections::HashMap;
use std::str::FromStr;
use crate::{Result, VdkError};

#[derive(Debug, Clone)]
pub struct MediaDescription {
    pub media_type: String,
    pub port: u16,
    pub protocol: String,
    pub format: String,
    pub attributes: HashMap<String, String>,
}

impl MediaDescription {
    pub fn new(media_type: &str, port: u16, protocol: &str, format: &str) -> Self {
        Self {
            media_type: media_type.to_string(),
            port,
            protocol: protocol.to_string(),
            format: format.to_string(),
            attributes: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionDescription {
    pub version: i32,
    pub origin: Option<String>,
    pub session_name: Option<String>,
    pub connection: Option<String>,
    pub time: Option<String>,
    pub attributes: HashMap<String, String>,
    pub media: Vec<MediaDescription>,
}

impl SessionDescription {
    pub fn new() -> Self {
        Self {
            version: 0,
            origin: None,
            session_name: None,
            connection: None,
            time: None,
            attributes: HashMap::new(),
            media: Vec::new(),
        }
    }

    pub fn parse(content: &str) -> Result<Self> {
        let mut sdp = SessionDescription::new();
        let mut current_media: Option<MediaDescription> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Each line must be of the form <type>=<value>
            let (typ, value) = match line.split_once('=') {
                Some((t, v)) if t.len() == 1 => (t, v.trim()),
                _ => return Err(VdkError::Protocol("Invalid SDP line format".into())),
            };

            match (typ, current_media.as_mut()) {
                ("v", _) => sdp.version = i32::from_str(value)?,
                ("o", _) => sdp.origin = Some(value.to_string()),
                ("s", _) => sdp.session_name = Some(value.to_string()),
                ("c", _) => sdp.connection = Some(value.to_string()),
                ("t", _) => sdp.time = Some(value.to_string()),
                ("m", _) => {
                    // If we were building a media section, add it to the list
                    if let Some(media) = current_media.take() {
                        sdp.media.push(media);
                    }

                    // Parse new media line: <media> <port> <proto> <fmt>
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    if parts.len() < 4 {
                        return Err(VdkError::Protocol("Invalid media description".into()));
                    }

                    let port = u16::from_str(parts[1])?;
                    current_media = Some(MediaDescription::new(
                        parts[0],
                        port,
                        parts[2],
                        parts[3],
                    ));
                },
                ("a", Some(media)) => {
                    // Attribute can be either a=<flag> or a=<name>:<value>
                    match value.split_once(':') {
                        Some((name, val)) => {
                            media.attributes.insert(name.to_string(), val.to_string());
                        }
                        None => {
                            media.attributes.insert(value.to_string(), "".to_string());
                        }
                    }
                },
                ("a", None) => {
                    // Session level attribute
                    match value.split_once(':') {
                        Some((name, val)) => {
                            sdp.attributes.insert(name.to_string(), val.to_string());
                        }
                        None => {
                            sdp.attributes.insert(value.to_string(), "".to_string());
                        }
                    }
                },
                _ => {} // Ignore unknown types
            }
        }

        // Add the last media section if any
        if let Some(media) = current_media {
            sdp.media.push(media);
        }

        Ok(sdp)
    }

    pub fn get_media(&self, media_type: &str) -> Option<&MediaDescription> {
        self.media.iter().find(|m| m.media_type == media_type)
    }

    pub fn get_attribute(&self, name: &str) -> Option<&String> {
        self.attributes.get(name)
    }
}

impl Default for SessionDescription {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sdp() {
        let sdp_str = "\
v=0
o=- 123 456 IN IP4 127.0.0.1
s=Test Session
c=IN IP4 127.0.0.1
t=0 0
m=video 5000 RTP/AVP 96
a=rtpmap:96 H264/90000
a=fmtp:96 profile-level-id=42e01f
m=audio 5002 RTP/AVP 97
a=rtpmap:97 MPEG4-GENERIC/44100/2
";

        let sdp = SessionDescription::parse(sdp_str).unwrap();
        
        assert_eq!(sdp.version, 0);
        assert_eq!(sdp.session_name, Some("Test Session".to_string()));
        assert_eq!(sdp.media.len(), 2);
        
        let video = sdp.get_media("video").unwrap();
        assert_eq!(video.port, 5000);
        assert_eq!(video.format, "96");
        assert!(video.attributes.contains_key("rtpmap"));
        
        let audio = sdp.get_media("audio").unwrap();
        assert_eq!(audio.port, 5002);
        assert_eq!(audio.format, "97");
    }
}
