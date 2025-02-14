use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc;
use crate::{Result, VdkError};
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

pub const DEFAULT_BUFFER_SIZE: usize = 8192;

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
    packet_tx: Option<mpsc::Sender<Vec<u8>>>,
}

#[derive(Debug)]
enum AuthMethod {
    None,
    Basic,
    Digest,
}

impl RTSPClient {
    pub fn new(url: &str) -> Result<Self> {
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
            packet_tx: Some(tx),
        })
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

    // Basic RTSP methods

    pub async fn connect(&mut self) -> Result<()> {
        let port = self.url.port().unwrap_or(554);
        let host = self.url.host_str()
            .ok_or_else(|| VdkError::Protocol("No host in URL".into()))?;

        self.connection = Some(RTSPConnection::connect(host, port).await?);
        Ok(())
    }

    pub async fn describe(&mut self) -> Result<Vec<MediaDescription>> {
        let request = self.build_request(
            "DESCRIBE",
            self.url.as_str(),
            &[("Accept", "application/sdp")]
        );
        
        let response = self.send_request(request).await?;

        // Find the SDP content
        let (_, body) = self.split_response(&response)?;
        let sdp_str = String::from_utf8_lossy(body);

        // Parse SDP content
        let mut media_descriptions = Vec::new();
        for line in sdp_str.lines() {
            if line.starts_with("m=") {
                if let Ok(media) = super::parse_sdp_media(&line[2..]) {
                    media_descriptions.push(media);
                }
            }
        }

        if media_descriptions.is_empty() {
            return Err(VdkError::Protocol("No media sections found in SDP".into()));
        }

        // Clear existing streams for new setup
        self.streams.clear();
        
        Ok(media_descriptions)
    }

    pub async fn setup(&mut self, media: &MediaDescription) -> Result<()> {
        // Create control URL
        let control = media.get_attribute("control")
            .ok_or_else(|| VdkError::Protocol("No control attribute in media".into()))?;

        let setup_url = if control.starts_with("rtsp://") {
            control.to_string()
        } else {
            format!("{}/{}", self.url.as_str().trim_end_matches('/'), control)
        };

        // Create initial transport
        let transport = TransportInfo::new_rtp_avp(self.next_client_ports()?);

        // Create media stream
        let stream = MediaStream::new(
            &media.media_type,
            control,
            transport.clone(),
            self.packet_tx.as_ref()
                .ok_or_else(|| VdkError::Protocol("No packet sender available".into()))?
                .clone(),
        );

        // Send SETUP request
        let request = self.build_request(
            "SETUP",
            &setup_url,
            &[("Transport", &stream.get_transport_str())]
        );
        
        let response = self.send_request(request).await?;

        let (headers, _) = self.split_response(&response)?;

        // Parse session and transport information
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

    pub async fn play(&mut self) -> Result<()> {
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
        
        let _response = self.send_request(request).await?;

        // Start receiving RTP packets.  Wrap socket in Arc<>.
        for stream in self.streams.values_mut() {
            if let Some(socket) = stream.rtp_socket.take() {
                let socket = Arc::new(socket);
                let packet_tx = stream.packet_sender.clone();
                let mut buffer = vec![0u8; DEFAULT_BUFFER_SIZE];
                
                tokio::spawn(async move {
                    loop {
                        // Use lock().await for async Mutex
                        match socket.recv_from(&mut buffer).await {
                            Ok((len, _addr)) => {
                                if packet_tx.send(buffer[..len].to_vec()).await.is_err() {
                                    break;
                                }
                            }
                            // WouldBlock means no data available, which is fine
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                tokio::task::yield_now().await;
                                continue;
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
        }

        Ok(())
    }

    // Helper methods for request handling

    fn build_request(&self, method: &str, path: &str, headers: &[(&str, &str)]) -> String {
        let mut request = format!("{} {} RTSP/1.0\r\n", method, path);
        request.push_str(&format!("CSeq: {}\r\n", self.cseq.fetch_add(1, Ordering::SeqCst)));
        request.push_str("User-Agent: vdkio\r\n");

        for &(name, value) in headers {
            request.push_str(&format!("{}: {}\r\n", name, value));
        }

        if let Some(ref session) = self.session {
            request.push_str(&format!("Session: {}\r\n", session));
        }

        request.push_str("\r\n");
        request
    }

    fn split_response<'a>(&self, response: &'a [u8]) -> Result<(&'a str, &'a [u8])> {
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

    async fn send_request(&mut self, request: String) -> Result<Vec<u8>> {
        let conn = self.connection.as_mut()
            .ok_or_else(|| VdkError::Protocol("Not connected".into()))?;

        conn.write_all(request.as_bytes()).await?;
        let response = conn.read_response().await?;

        let (headers, _) = self.split_response(&response)?;
        let status = headers.lines().next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u32>().ok())
            .ok_or_else(|| VdkError::Protocol("Invalid response status".into()))?;

        match status {
            200 => Ok(response),
            401 => {
                self.handle_auth(headers, &response).await
            }
            _ => Err(VdkError::Protocol(format!("Request failed with status {}", status))),
        }
    }

    async fn handle_auth(&mut self, headers: &str, response: &[u8]) -> Result<Vec<u8>> {
        self.parse_auth_challenge(response)?;
        let (method, url) = self.get_method_and_url(headers)?;
        let auth_request = self.build_authenticated_request(method, url)?;
        
        let conn = self.connection.as_mut()
            .ok_or_else(|| VdkError::Protocol("Not connected".into()))?;

        conn.write_all(auth_request.as_bytes()).await?;
        let auth_response = conn.read_response().await?;
        
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

    fn get_method_and_url<'a>(&self, headers: &'a str) -> Result<(&'a str, &'a str)> {
        let status_line = headers.lines().next()
            .ok_or_else(|| VdkError::Protocol("Empty response".into()))?;

        let mut parts = status_line.split_whitespace();
        let method = parts.next().ok_or_else(|| VdkError::Protocol("Missing method".into()))?;
        let url = parts.next().ok_or_else(|| VdkError::Protocol("Missing URL".into()))?;

        Ok((method, url))
    }

    fn parse_auth_challenge(&mut self, response: &[u8]) -> Result<()> {
        let (headers, _) = self.split_response(response)?;

        for line in headers.lines() {
            if line.starts_with("WWW-Authenticate: ") {
                let auth_header = &line["WWW-Authenticate: ".len()..];
                if auth_header.starts_with("Digest ") {
                    self.auth_method = AuthMethod::Digest;
                    let parts: HashMap<_, _> = auth_header["Digest ".len()..]
                        .split(',')
                        .filter_map(|part| {
                            let mut parts = part.trim().splitn(2, '=');
                            Some((
                                parts.next()?.trim(),
                                parts.next()?.trim_matches('"').trim()
                            ))
                        })
                        .collect();

                    self.realm = parts.get("realm").map(|&s| s.to_string());
                    self.nonce = parts.get("nonce").map(|&s| s.to_string());
                } else if auth_header.starts_with("Basic ") {
                    self.auth_method = AuthMethod::Basic;
                }
                return Ok(());
            }
        }
        
        Err(VdkError::Protocol("No authentication challenge found".into()))
    }

    fn build_authenticated_request(&self, method: &str, url: &str) -> Result<String> {
       match self.auth_method {
           AuthMethod::Digest => {
                let (username, password) = self.get_credentials()?;
                let realm = self.realm.as_deref().unwrap_or("RTSP Server");
                let nonce = self.nonce.as_deref().unwrap_or("none");

                let ha1 = md5_hash(&format!("{}:{}:{}", username, realm, password));
                let ha2 = md5_hash(&format!("{}:{}", method, url));
                let response = md5_hash(&format!("{}:{}:{}", ha1, nonce, ha2));

                let auth_header = format!(
                    r#"Digest username="{}", realm="{}", nonce="{}", uri="{}", response="{}""#,
                    username, realm, nonce, url, response
                );

                let mut request = format!("{} {} RTSP/1.0\r\n", method, url);
                request.push_str(&format!("CSeq: {}\r\n", self.cseq.fetch_add(1, Ordering::SeqCst)));
                request.push_str("User-Agent: vdkio\r\n");
                request.push_str(&format!("Authorization: {}\r\n", auth_header));
                request.push_str("\r\n");
                Ok(request)
           }
           AuthMethod::Basic => {
                let (username, password) = self.get_credentials()?;
                let auth = base64::engine::general_purpose::STANDARD.encode(
                    format!("{}:{}", username, password).as_bytes()
                );
                let auth_header = format!("Basic {}", auth);
                let mut request = format!("{} {} RTSP/1.0\r\n", method, url);
                request.push_str(&format!("CSeq: {}\r\n", self.cseq.fetch_add(1, Ordering::SeqCst)));
                request.push_str("User-Agent: vdkio\r\n");
                request.push_str(&format!("Authorization: {}\r\n", auth_header));
                request.push_str("\r\n");
                Ok(request)
           }
           AuthMethod::None => {
                Err(VdkError::Protocol("Authentication required but no credentials available".into()))
           }
       }
    }

    fn get_credentials(&self) -> Result<(&str, &str)> {
        match (&self.username, &self.password) {
            (Some(username), Some(password)) => Ok((username, password)),
            _ => Err(VdkError::Protocol("No credentials available".into())),
        }
    }

    pub async fn teardown(&mut self) -> Result<()> {
        if let Some(ref session) = self.session {
            let request = self.build_request(
                "TEARDOWN",
                self.url.as_str(),
                &[("Session", session)]
            );
            
            self.send_request(request).await?;
        }

        self.streams.clear();
        self.session = None;
        Ok(())
    }

    fn next_client_ports(&self) -> Result<(u16, u16)> {
        // Simple port allocation strategy - in real implementation this should be more sophisticated
        let base_port = 5000 + (self.streams.len() * 2) as u16;
        if base_port > 65530 {
            return Err(VdkError::Protocol("No more ports available".into()));
        }
        Ok((base_port, base_port + 1))
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
        
        // if let Some(video) = sdp.get(0) {
        //     client.setup(video).await.unwrap();
        // }
        
        // if let Some(audio) = sdp.get(1) {
        //     client.setup(audio).await.unwrap();
        // }
        
        // client.play().await.unwrap();
        
        // // Test receiving packets
        // if let Some(mut rx) = client.get_packet_receiver() {
        //     tokio::spawn(async move {
        //         while let Some(packet) = rx.recv().await {
        //             println!("Received packet of size: {}", packet.len());
        //         }
        //     });
        // }
        
        // sleep(Duration::from_secs(1)).await;
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
