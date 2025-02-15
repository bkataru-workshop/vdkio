use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc;
use crate::{Result as VdkResult, VdkError};
use super::{
    connection::RTSPConnection,
    stream::MediaStream,
    transport::TransportInfo,
    MediaDescription,
};
use url::Url;
use base64;
use base64::Engine as _;
use md5::{Md5, Digest};
use std::sync::Arc;
use chrono::Utc;
use tokio::time::Duration;

pub const DEFAULT_BUFFER_SIZE: usize = 8192;

#[derive(Debug, Clone, Default)]
pub struct RTSPSetupOptions {
    pub enable_video: bool,
    pub enable_audio: bool,
    pub video_codec_filter: Option<String>,
    pub audio_codec_filter: Option<String>,
    pub receive_buffer_size: usize,
}

impl RTSPSetupOptions {
    pub fn new() -> Self {
        Self {
            enable_video: true,
            enable_audio: true,
            video_codec_filter: None,
            audio_codec_filter: None,
            receive_buffer_size: DEFAULT_BUFFER_SIZE,
        }
    }

    pub fn with_video(mut self, enable: bool) -> Self {
        self.enable_video = enable;
        self
    }

    pub fn with_audio(mut self, enable: bool) -> Self {
        self.enable_audio = enable;
        self
    }

    pub fn with_video_codec(mut self, codec: &str) -> Self {
        self.video_codec_filter = Some(codec.to_string());
        self
    }

    pub fn with_audio_codec(mut self, codec: &str) -> Self {
        self.audio_codec_filter = Some(codec.to_string());
        self
    }
     pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.receive_buffer_size = size;
        self
    }
}

#[derive(Debug)]
enum AuthMethod {
    None,
    Basic,
    Digest,
}

#[derive(Debug)]
pub struct RTSPClient {
    connection: Option<RTSPConnection>,
    url: Url,
    cseq: AtomicU32,
    session: Option<String>,
    streams: HashMap<String, MediaStream>,
    username: Option<String>,
    password: Option<String>,
    auth_method: AuthMethod,
    realm: Option<String>,
    nonce: Option<String>,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    reconnect_delay: Duration,
    packet_tx: Option<mpsc::Sender<Vec<u8>>>,
    last_request: Option<(String, String)>,
}

impl RTSPClient {
    pub fn new(url: &str) -> VdkResult<Self> {
        let parsed_url = Url::parse(url)
            .map_err(|e| VdkError::Protocol(format!("Invalid URL: {}", e)))?;

        if parsed_url.scheme() != "rtsp" {
            return Err(VdkError::Protocol("URL scheme is not 'rtsp'".into()));
        }

        let (tx, _) = mpsc::channel(100);

        Ok(Self {
            connection: None,
            url: parsed_url.clone(),
            cseq: AtomicU32::new(1),
            session: None,
            streams: HashMap::new(),
            username: parsed_url.username().is_empty().then(|| None).unwrap_or_else(|| Some(parsed_url.username().to_string())),
            password: parsed_url.password().map(String::from),
            auth_method: AuthMethod::None,
            realm: None,
            nonce: None,
            reconnect_attempts: 0,
            max_reconnect_attempts: 3,
            reconnect_delay: Duration::from_secs(1),
            packet_tx: Some(tx),
            last_request: None,
        })
    }

    pub async fn connect(&mut self) -> VdkResult<()> {
        let port = self.url.port().unwrap_or(554);
        let host = self.url.host_str()
            .ok_or_else(|| VdkError::Protocol("No host in URL".into()))?;

        self.connection = Some(RTSPConnection::connect(host, port).await?);
        Ok(())
    }

     pub async fn reconnect(&mut self) -> VdkResult<bool> {
        if self.reconnect_attempts >= self.max_reconnect_attempts {
            return Ok(false);
        }

        println!("Attempting reconnection ({}/{})", 
            self.reconnect_attempts + 1, 
            self.max_reconnect_attempts
        );

        tokio::time::sleep(self.reconnect_delay).await;
        
        match self.connect().await {
            Ok(_) => {
                println!("Reconnection successful");
                self.reconnect_attempts = 0;
                Ok(true)
            }
            Err(e) => {
                println!("Reconnection failed: {}", e);
                self.reconnect_attempts += 1;
                self.reconnect_delay *= 2; // Exponential backoff
                Ok(false)
            }
        }
    }

    pub async fn describe(&mut self) -> VdkResult<Vec<MediaDescription>> {
        let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let headers = [
            ("Accept", "application/sdp"),
            ("Date", &date),
            ("User-Agent", "vdkio/1.0"),
            ("Range", "npt=0.000-"),
        ];

        let request = self.build_request("DESCRIBE", self.url.as_str(), &headers);
        println!("Sending DESCRIBE request:\n{}", request);
        let response = self.send_request(&request).await?;

        // Find the SDP content
        let (_, body) = self.split_response(&response)?;
        let sdp_str = String::from_utf8_lossy(body);
        println!("Parsing SDP:\n{}", sdp_str);

        // Parse SDP content with enhanced attributes support
        let mut media_descriptions: Vec<MediaDescription> = Vec::new();
        let mut global_attrs: HashMap<String, String> = HashMap::new();
        let mut current_media: Option<MediaDescription> = None;

        // Parse SDP line by line
        for line in sdp_str.lines().filter(|l| !l.is_empty()) {
            match line.chars().next() {
                Some('m') if line.starts_with("m=") => {
                    // Save previous media section if exists
                    if let Some(media) = current_media.take() {
                        media_descriptions.push(media);
                    }
                    
                    // Parse new media section
                    if let Ok(media) = super::parse_sdp_media(&line[2..]) {
                        current_media = Some(media);
                    }
                },
                Some('a') if line.starts_with("a=") => {
                    if let Some(attr_str) = line.strip_prefix("a=") {
                        let (name, value) = if let Some((n, v)) = attr_str.split_once(':') {
                            (n, v)
                        } else {
                            (attr_str, "")
                        };

                        // Add to current media or global attributes
                        if let Some(ref mut media) = current_media {
                            media.set_attribute(name, value);
                        } else {
                            global_attrs.insert(name.to_string(), value.to_string());
                        }
                    }
                },
                _ => continue,
            }
        }

        // Add last media section if exists
        if let Some(media) = current_media {
            media_descriptions.push(media);
        }

        if media_descriptions.is_empty() {
            return Err(VdkError::Protocol("No media sections found in SDP".into()));
        }

        // Process global control and apply to media sections
        let base_control = global_attrs.get("control").map(|s| s.as_str()).unwrap_or("*");
        let base_url = if base_control == "*" {
            self.url.as_str().trim_end_matches('/')
        } else {
            base_control.trim_end_matches('/')
        };

        // Update each media section's control URL
        for media in &mut media_descriptions {
            if let Some(track_control) = media.get_attribute("control").cloned() {
                let full_control = if track_control.contains("://") {
                    track_control
                } else if track_control.starts_with('/') {
                    format!("{}{}", base_url, track_control)
                } else {
                    format!("{}/{}", base_url, track_control)
                };
                media.set_attribute("control", &full_control);
                println!("Media control URL: {}", full_control);
            }
        }

        println!("Found {} media descriptions", media_descriptions.len());
        self.streams.clear();
        Ok(media_descriptions)
    }

    pub async fn setup(&mut self, media: &MediaDescription) -> VdkResult<()> {
        let control = media.get_attribute("control")
            .ok_or_else(|| VdkError::Protocol("No control attribute in media".into()))?;

        let setup_url = if control.starts_with("rtsp://") {
            control.to_string()
        } else {
            format!("{}/{}", self.url.as_str().trim_end_matches('/'), control)
        };

        let transport = TransportInfo::new_rtp_avp(self.next_client_ports()?);

        let stream = MediaStream::new(
            &media.media_type,
            control,
            transport.clone(),
            self.packet_tx.as_ref()
                .ok_or_else(|| VdkError::Protocol("No packet sender available".into()))?
                .clone(),
        );

        let request = self.build_request(
            "SETUP",
            &setup_url,
            &[("Transport", &stream.get_transport_str())]
        );
        
        let response = self.send_request(&request).await?;
        let (headers, _) = self.split_response(&response)?;

        for line in headers.lines() {
            if line.starts_with("Session: ") {
                self.session = Some(line[9..].trim().to_string());
            }
            if line.starts_with("Transport: ") {
                if let Some(updated_transport) = TransportInfo::parse(&line[11..]) {
                    let mut stream = stream;
                    stream.transport = updated_transport;
                    stream.setup_transport().await?;
                    self.streams.insert(media.media_type.clone(), stream);
                    return Ok(());
                }
            }
        }

        Err(VdkError::Protocol("Failed to setup media stream".into()))
    }

    pub async fn play(&mut self) -> VdkResult<()> {
        let session = self.session.as_ref()
            .ok_or_else(|| VdkError::Protocol("No session established".into()))?;

        let request = self.build_request(
            "PLAY",
            self.url.as_str(),
            &[
                ("Session", session),
                ("Range", "npt=0.000-")
            ]
        );
        
        let _response = self.send_request(&request).await?;

        for stream in self.streams.values_mut() {
            if let Some(socket) = stream.rtp_socket.take() {
                let socket = Arc::new(socket);
                let packet_tx = stream.packet_sender.clone();
                let mut buffer = vec![0u8; DEFAULT_BUFFER_SIZE];
                
                tokio::spawn(async move {
                    loop {
                        match socket.recv_from(&mut buffer).await {
                            Ok((len, _addr)) => {
                                match packet_tx.send(buffer[..len].to_vec()).await {
                                    Ok(_) => continue,
                                    Err(e) => {
                                        println!("Failed to send packet: {}", e);
                                        break;
                                    }
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                                println!("Socket timeout, attempting reconnect");
                                tokio::task::yield_now().await;
                                continue;
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                                println!("Socket interrupted, continuing");
                                tokio::task::yield_now().await;
                                continue;
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                tokio::task::yield_now().await;
                                continue;
                            }
                            Err(e) => {
                                println!("Socket error: {}", e);
                                break;
                            }
                        }
                    }
                });
            }
        }

        Ok(())
    }

    pub fn get_packet_receiver(&mut self) -> Option<mpsc::Receiver<Vec<u8>>> {
        if let Some(_tx) = self.packet_tx.take() {
            let (new_tx, rx) = mpsc::channel(100);
            self.packet_tx = Some(new_tx);
            Some(rx)
        } else {
            None
        }
    }

    fn build_request(&self, method: &str, path: &str, headers: &[(&str, &str)]) -> String {
        let mut request = format!("{} {} RTSP/1.0\r\n", method, path);
        request.push_str(&format!("CSeq: {}\r\n", self.cseq.fetch_add(1, Ordering::SeqCst)));
        request.push_str("User-Agent: vdkio/1.0\r\n");
        
        for &(name, value) in headers {
            request.push_str(&format!("{}: {}\r\n", name, value));
        }

        if let Some(ref session) = self.session {
            request.push_str(&format!("Session: {}\r\n", session));
        }

        request.push_str("\r\n");
        request
    }

    fn split_response<'a>(&self, response: &'a [u8]) -> VdkResult<(&'a str, &'a [u8])> {
        for i in 0..response.len()-3 {
            if &response[i..i+4] == b"\r\n\r\n" {
                let headers = std::str::from_utf8(&response[..i])
                    .map_err(|_| VdkError::Protocol("Invalid UTF-8 in headers".into()))?;
                let body = &response[i+4..];
                return Ok((headers, body));
            }
        }
        Err(VdkError::Protocol("No header/body boundary found".into()))
    }

    async fn send_request(&mut self, request: &str) -> VdkResult<Vec<u8>> {
        let conn = self.connection.as_mut()
            .ok_or_else(|| VdkError::Protocol("Not connected".into()))?;

        // Extract and store method and URL from request
        let first_line = request.lines().next()
            .ok_or_else(|| VdkError::Protocol("Invalid request format".into()))?;
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            self.last_request = Some((parts[0].to_string(), parts[1].to_string()));
        }

        println!("Sending request:\n{}", request);
        conn.write_all(request.as_bytes()).await?;
        let response = conn.read_response().await?;
        println!("Received response:\n{}", String::from_utf8_lossy(&response));

        let (headers, _) = self.split_response(&response)?;
        let status = headers.lines().next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u32>().ok())
            .ok_or_else(|| VdkError::Protocol("Invalid response status".into()))?;

        match status {
            200 => Ok(response),
            401 => self.handle_auth(headers, &response).await,
            _ => Err(VdkError::Protocol(format!("Request failed with status {}", status))),
        }
    }

    async fn handle_auth(&mut self, _headers: &str, response: &[u8]) -> VdkResult<Vec<u8>> {
        println!("Handling auth challenge...");
        
        // Parse WWW-Authenticate header and set up auth method
        self.parse_auth_challenge(response)?;

        // Get original request details from stored values
        let (method, url) = self.last_request.as_ref()
            .ok_or_else(|| VdkError::Protocol("No previous request found".into()))?;
        println!("Original request was: {} {}", method, url);

        // Build new authenticated request
        let auth_request = self.build_authenticated_request(method, url)?;
        println!("Sending authenticated request:\n{}", &auth_request);

        let conn = self.connection.as_mut()
            .ok_or_else(|| VdkError::Protocol("Not connected".into()))?;

        // Send authenticated request
        conn.write_all(auth_request.as_bytes()).await?;
        let auth_response = conn.read_response().await?;
        println!("Received auth response:\n{}", String::from_utf8_lossy(&auth_response));

        // Check if authentication succeeded
        let (headers, _) = self.split_response(&auth_response)?;
        let status = headers.lines().next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u32>().ok())
            .ok_or_else(|| VdkError::Protocol("Invalid response status".into()))?;

        if status == 200 {
            Ok(auth_response)
        } else {
            Err(VdkError::Protocol(format!("Authentication failed with status {}", status)))
        }
    }

    fn parse_auth_challenge(&mut self, response: &[u8]) -> VdkResult<()> {
        let (headers, _) = self.split_response(response)?;

        for line in headers.lines() {
            if line.starts_with("WWW-Authenticate: ") {
                let auth_header = &line["WWW-Authenticate: ".len()..];
                println!("Auth header: {}", auth_header);

                if auth_header.starts_with("Digest ") {
                    self.auth_method = AuthMethod::Digest;
                    println!("Parsing Digest authentication...");

                    let parts: HashMap<_, _> = auth_header["Digest ".len()..]
                        .split(',')
                        .filter_map(|part| {
                            let mut parts = part.trim().splitn(2, '=');
                            let key = parts.next()?.trim();
                            let value = parts.next()?.trim_matches('"').trim();
                            println!("Auth parameter: {} = {}", key, value);
                            Some((key, value))
                        })
                        .collect();

                    self.realm = parts.get("realm").map(|&s| s.to_string());
                    self.nonce = parts.get("nonce").map(|&s| s.to_string());
                    println!("Parsed Digest auth - realm: {:?}, nonce: {:?}", self.realm, self.nonce);
                    return Ok(());
                } else if auth_header.starts_with("Basic ") {
                    self.auth_method = AuthMethod::Basic;
                    return Ok(());
                }
            }
        }
        
        Err(VdkError::Protocol("No authentication challenge found".into()))
    }

    fn build_authenticated_request(&self, method: &str, url: &str) -> VdkResult<String> {
        match self.auth_method {
            AuthMethod::Digest => {
                let (username, password) = self.get_credentials()?;
                let realm = self.realm.as_deref()
                    .ok_or_else(|| VdkError::Protocol("No realm in auth challenge".into()))?;
                let nonce = self.nonce.as_deref()
                    .ok_or_else(|| VdkError::Protocol("No nonce in auth challenge".into()))?;

                println!("Building Digest auth with realm '{}' and nonce '{}'", realm, nonce);

                let ha1 = md5_hash(&format!("{}:{}:{}", username, realm, password));
                let ha2 = md5_hash(&format!("{}:{}", method, url));
                let response = md5_hash(&format!("{}:{}:{}", ha1, nonce, ha2));

                let auth_header = format!(
                    r#"Digest username="{}", realm="{}", nonce="{}", uri="{}", response="{}""#,
                    username, realm, nonce, url, response
                );

                let mut request = format!("{} {} RTSP/1.0\r\n", method, url);
                request.push_str(&format!("CSeq: {}\r\n", self.cseq.fetch_add(1, Ordering::SeqCst)));
                request.push_str("User-Agent: vdkio/1.0\r\n");
                request.push_str(&format!("Authorization: {}\r\n", auth_header));
                request.push_str("\r\n");

                println!("Built Digest auth request:\n{}", request);
                Ok(request)
            }
            AuthMethod::Basic => {
                let (username, password) = self.get_credentials()?;
                println!("Building Basic auth for user '{}'", username);

                let auth = base64::engine::general_purpose::STANDARD.encode(
                    format!("{}:{}", username, password).as_bytes()
                );
                let auth_header = format!("Basic {}", auth);
                let mut request = format!("{} {} RTSP/1.0\r\n", method, url);
                request.push_str(&format!("CSeq: {}\r\n", self.cseq.fetch_add(1, Ordering::SeqCst)));
                request.push_str("User-Agent: vdkio/1.0\r\n");
                request.push_str(&format!("Authorization: {}\r\n", auth_header));
                request.push_str("\r\n");

                println!("Built Basic auth request:\n{}", request);
                Ok(request)
            }
            AuthMethod::None => {
                Err(VdkError::Protocol("Authentication required but no credentials available".into()))
            }
        }
    }

    fn get_credentials(&self) -> VdkResult<(&str, &str)> {
        match (&self.username, &self.password) {
            (Some(username), Some(password)) => Ok((username, password)),
            _ => Err(VdkError::Protocol("No credentials available".into())),
        }
    }

    fn next_client_ports(&self) -> VdkResult<(u16, u16)> {
        let base_port = 5000 + (self.streams.len() * 2) as u16;
        if base_port > 65530 {
            return Err(VdkError::Protocol("No more ports available".into()));
        }
        Ok((base_port, base_port + 1))
    }

    pub async fn teardown(&mut self) -> VdkResult<()> {
        if let Some(ref session) = self.session {
            let request = self.build_request(
                "TEARDOWN",
                self.url.as_str(),
                &[("Session", session)]
            );
            
            let _response = self.send_request(&request).await?;
        }

        self.streams.clear();
        self.session = None;
        Ok(())
    }
}

fn md5_hash(s: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[allow(dead_code)]    
    async fn test_rtsp_client_lifecycle() {
        let url = "rtsp://example.com/stream";
        let mut client = RTSPClient::new(&url).unwrap();
        assert!(client.connect().await.is_err());
        let sdp = client.describe().await;
        assert!(sdp.is_err());
    }

    #[test]
    #[allow(dead_code)]    
    fn test_url_parsing() {
        assert!(RTSPClient::new("rtsp://example.com/stream").is_ok());
        assert!(RTSPClient::new("rtsp://user:pass@example.com:8554/stream").is_ok());
        assert!(RTSPClient::new("http://example.com").is_err());
        assert!(RTSPClient::new("not a url").is_err());
    }

    #[test]
    #[allow(dead_code)]    
    fn test_request_building() {
        let client = RTSPClient::new("rtsp://example.com/stream").unwrap();
        
        let request = client.build_request(
            "DESCRIBE",
            "rtsp://example.com/stream",
            &[("Accept", "application/sdp")]
        );
        
        assert!(request.starts_with("DESCRIBE rtsp://example.com/stream RTSP/1.0\r\n"));
        assert!(request.contains("Accept: application/sdp\r\n"));
        assert!(request.ends_with("\r\n"));
        assert!(request.contains("CSeq: 1\r\n"));
        
        let second_request = client.build_request(
            "SETUP",
            "rtsp://example.com/stream",
            &[("Transport", "RTP/AVP;unicast")]
        );
        assert!(second_request.contains("CSeq: 2\r\n"));
    }

    #[test]
    #[allow(dead_code)]    
    fn test_split_response() {
        let client = RTSPClient::new("rtsp://example.com/stream").unwrap();
        let response = b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Type: application/sdp\r\n\r\nbody";
        let (headers, body) = client.split_response(response).unwrap();
        assert_eq!(headers, "RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Type: application/sdp");
        assert_eq!(body, b"body");

        let response_no_body = b"RTSP/1.0 200 OK\r\nCSeq: 1\r\n\r\n";
        let (headers_no_body, body_no_body) = client.split_response(response_no_body).unwrap();
        assert_eq!(headers_no_body, "RTSP/1.0 200 OK\r\nCSeq: 1");
        assert_eq!(body_no_body, b"");
    }
}
