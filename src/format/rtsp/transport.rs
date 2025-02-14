use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TransportInfo {
    pub protocol: String,
    pub cast_type: CastType,
    pub client_port_rtp: Option<u16>,
    pub client_port_rtcp: Option<u16>,
    pub server_port_rtp: Option<u16>,
    pub server_port_rtcp: Option<u16>,
    pub ssrc: Option<u32>,
    pub mode: Option<String>,
    pub extra_params: HashMap<String, Option<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CastType {
    Unicast,
    Multicast,
}

impl TransportInfo {
    /// Create a new RTP/AVP transport with client ports
    pub fn new_rtp_avp(ports: (u16, u16)) -> Self {
        Self {
            protocol: "RTP/AVP".to_string(),
            cast_type: CastType::Unicast,
            client_port_rtp: Some(ports.0),
            client_port_rtcp: Some(ports.1),
            server_port_rtp: None,
            server_port_rtcp: None,
            ssrc: None,
            mode: None,
            extra_params: HashMap::new(),
        }
    }

    /// Parse a transport string into TransportInfo
    pub fn parse(transport: &str) -> Option<Self> {
        let mut info = TransportInfo {
            protocol: String::new(),
            cast_type: CastType::Unicast,
            client_port_rtp: None,
            client_port_rtcp: None,
            server_port_rtp: None,
            server_port_rtcp: None,
            ssrc: None,
            mode: None,
            extra_params: HashMap::new(),
        };

        let parts: Vec<&str> = transport.split(';').collect();
        if parts.is_empty() {
            return None;
        }

        info.protocol = parts[0].trim().to_string();

        for part in parts.iter().skip(1) {
            let part = part.trim();
            if part == "unicast" {
                info.cast_type = CastType::Unicast;
            } else if part == "multicast" {
                info.cast_type = CastType::Multicast;
            } else if let Some((key, value)) = part.split_once('=') {
                match key {
                    "client_port" => {
                        if let Some((rtp, rtcp)) = value.split_once('-') {
                            info.client_port_rtp = rtp.parse().ok();
                            info.client_port_rtcp = rtcp.parse().ok();
                        }
                    }
                    "server_port" => {
                        if let Some((rtp, rtcp)) = value.split_once('-') {
                            info.server_port_rtp = rtp.parse().ok();
                            info.server_port_rtcp = rtcp.parse().ok();
                        }
                    }
                    "ssrc" => {
                        if let Ok(ssrc) = u32::from_str_radix(value.trim_start_matches("0x"), 16) {
                            info.ssrc = Some(ssrc);
                        }
                    }
                    "mode" => {
                        info.mode = Some(value.to_string());
                    }
                    _ => {
                        info.extra_params.insert(key.to_string(), Some(value.to_string()));
                    }
                }
            } else {
                info.extra_params.insert(part.to_string(), None);
            }
        }

        Some(info)
    }

    /// Generate a transport header string
    pub fn to_string(&self) -> String {
        let mut parts = vec![self.protocol.clone()];
        
        parts.push(match self.cast_type {
            CastType::Unicast => "unicast",
            CastType::Multicast => "multicast",
        }.to_string());

        if let (Some(rtp), Some(rtcp)) = (self.client_port_rtp, self.client_port_rtcp) {
            parts.push(format!("client_port={}-{}", rtp, rtcp));
        }

        if let (Some(rtp), Some(rtcp)) = (self.server_port_rtp, self.server_port_rtcp) {
            parts.push(format!("server_port={}-{}", rtp, rtcp));
        }

        if let Some(ssrc) = self.ssrc {
            parts.push(format!("ssrc={:08x}", ssrc));
        }

        if let Some(ref mode) = self.mode {
            parts.push(format!("mode={}", mode));
        }

        for (key, value) in &self.extra_params {
            if let Some(val) = value {
                parts.push(format!("{}={}", key, val));
            } else {
                parts.push(key.clone());
            }
        }

        parts.join(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_rtp_avp() {
        let transport = TransportInfo::new_rtp_avp((5000, 5001));
        assert_eq!(transport.protocol, "RTP/AVP");
        assert_eq!(transport.cast_type, CastType::Unicast);
        assert_eq!(transport.client_port_rtp, Some(5000));
        assert_eq!(transport.client_port_rtcp, Some(5001));
    }

    #[test]
    fn test_transport_parse_basic() {
        let transport = "RTP/AVP;unicast;client_port=5000-5001";
        let info = TransportInfo::parse(transport).unwrap();
        assert_eq!(info.protocol, "RTP/AVP");
        assert_eq!(info.cast_type, CastType::Unicast);
        assert_eq!(info.client_port_rtp, Some(5000));
        assert_eq!(info.client_port_rtcp, Some(5001));
    }

    #[test]
    fn test_transport_parse_full() {
        let transport = "RTP/AVP;unicast;client_port=5000-5001;server_port=6000-6001;ssrc=0x12345678;mode=play";
        let info = TransportInfo::parse(transport).unwrap();
        assert_eq!(info.protocol, "RTP/AVP");
        assert_eq!(info.cast_type, CastType::Unicast);
        assert_eq!(info.client_port_rtp, Some(5000));
        assert_eq!(info.client_port_rtcp, Some(5001));
        assert_eq!(info.server_port_rtp, Some(6000));
        assert_eq!(info.server_port_rtcp, Some(6001));
        assert_eq!(info.ssrc, Some(0x12345678));
        assert_eq!(info.mode, Some("play".to_string()));
    }

    #[test]
    fn test_transport_parse_multicast() {
        let transport = "RTP/AVP;multicast;port=5000-5001;ttl=32";
        let info = TransportInfo::parse(transport).unwrap();
        assert_eq!(info.cast_type, CastType::Multicast);
        assert!(info.extra_params.contains_key("ttl"));
    }

    #[test]
    fn test_transport_to_string() {
        let transport = TransportInfo::new_rtp_avp((5000, 5001));
        let transport_str = transport.to_string();
        assert!(transport_str.contains("RTP/AVP"));
        assert!(transport_str.contains("unicast"));
        assert!(transport_str.contains("client_port=5000-5001"));
    }
}