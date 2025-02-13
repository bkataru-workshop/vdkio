use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use crate::{Result, VdkError};
use super::{connection::RTSPConnection, sdp::SessionDescription};
use url::Url;
use base64;
use base64::Engine as _;
use md5::{Md5, Digest};

#[derive(Debug)]
pub struct RTSPClient {
    connection: Option<RTSPConnection>,
    url: Url,
    cseq: AtomicU32,
    session: Option<String>,
    streams: HashMap<String, StreamInfo>,
    username: Option<String>,
    password: Option<String>,
    auth_method: AuthMethod,
    realm: Option<String>,
    nonce: Option<String>,
}

#[derive(Debug)]
enum AuthMethod {
    None,
    Basic,
    Digest,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_rtsp_client_lifecycle() {
        let url = "rtsp://example.com/stream";
        let mut client = RTSPClient::new(&url).unwrap();
        
        client.connect().await.unwrap();
        
        let sdp = client.describe().await.unwrap();
        assert!(!sdp.media.is_empty(), "No media sections in SDP");
        
        if let Some(video) = sdp.get_media("video") {
            client.setup("video").await.unwrap();
            assert_eq!(video.protocol, "RTP/AVP");
        }
        
        if let Some(audio) = sdp.get_media("audio") {
            client.setup("audio").await.unwrap();
            assert_eq!(audio.protocol, "RTP/AVP");
        }
        
        client.play().await.unwrap();
        
        sleep(Duration::from_secs(1)).await;
    }

    #[test]
    fn test_url_parsing() {
        assert!(RTSPClient::new("rtsp://example.com/stream").is_ok());
        assert!(RTSPClient::new("rtsp://user:pass@example.com:8554/stream").is_ok());
        assert!(RTSPClient::new("http://example.com").is_err());
        assert!(RTSPClient::new("not a url").is_err());
    }

    #[test]
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
}

#[derive(Debug)]
struct StreamInfo {
    control: String,
    transport: Option<String>,
}

impl RTSPClient {
    pub fn new(url: &str) -> Result<Self> {
        let parsed_url = Url::parse(url)
            .map_err(|e| VdkError::Protocol(format!("Invalid URL: {}", e)))?;

        let username = if !parsed_url.username().is_empty() {
            Some(parsed_url.username().to_string())
        } else {
            None
        };

        let password = parsed_url.password().map(|s| s.to_string());

        if parsed_url.scheme() != "rtsp" {
            return Err(VdkError::Protocol("URL scheme is not 'rtsp'".into()));
        }

        // Set initial auth method based on credentials
        let auth_method = if username.is_some() && password.is_some() {
            AuthMethod::Basic // Start with Basic, can switch to Digest if needed
        } else {
            AuthMethod::None
        };

        log::debug!("Creating client with URL - scheme: {}, host: {:?}, auth: {}:{}",
            parsed_url.scheme(), parsed_url.host_str(),
            username.as_deref().unwrap_or("none"), password.as_deref().unwrap_or("none"));

        Ok(Self {
            connection: None,
            url: parsed_url,
            cseq: AtomicU32::new(1),
            session: None,
            streams: HashMap::new(),
            username,
            password,
            auth_method,
            realm: None,
            nonce: None,
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        let port = self.url.port().unwrap_or(554);
        let host = self.url.host_str()
            .ok_or_else(|| VdkError::Protocol("No host in URL".into()))?;

        self.connection = Some(RTSPConnection::connect(host, port).await?);
        Ok(())
    }

    pub async fn describe(&mut self) -> Result<SessionDescription> {
        log::debug!("Starting DESCRIBE request");

        let response = self.send_request_with_auth("DESCRIBE", &self.url.to_string(), &[
            ("Accept", "application/sdp")
        ]).await?;

        let (_headers, body) = self.split_response(&response)?;
        if body.is_empty() {
            return Err(VdkError::Protocol("No SDP content in DESCRIBE response".into()));
        }

        let sdp = SessionDescription::parse(&String::from_utf8_lossy(body))?;

        for media in &sdp.media {
            let control = media.attributes.get("control")
                .cloned()
                .unwrap_or_else(|| format!("trackID={}", media.format));

            self.streams.insert(media.media_type.clone(), StreamInfo {
                control,
                transport: None,
            });
        }

        Ok(sdp)
    }

    pub async fn setup(&mut self, media_type: &str) -> Result<()> {
        let stream = self.streams.get(media_type)
            .ok_or_else(|| VdkError::Protocol(format!("No {} stream found", media_type)))?;
            
        let mut setup_url = self.url.clone();
        setup_url.set_path(&stream.control);

        let response = self.send_request_with_auth("SETUP", &setup_url.to_string(), &[
            ("Transport", "RTP/AVP;unicast;client_port=0-1")]).await?;

        let (headers, _) = self.split_response(&response)?;
        for line in headers.lines() {
            if line.starts_with("Session: ") {
                self.session = Some(line[9..].trim().to_string());
            }
            if line.starts_with("Transport: ") {
                if let Some(stream_info) = self.streams.get_mut(media_type) {
                    stream_info.transport = Some(line[11..].trim().to_string());
                }
            }
        }

        Ok(())
    }

    pub async fn play(&mut self) -> Result<()> {
        let session = self.session.as_ref()
            .ok_or_else(|| VdkError::Protocol("No session established".into()))?;
        let session_clone = session.clone();

        let _response = self.send_request_with_auth("PLAY", &self.url.to_string(), &[
            ("Session", &session_clone), ("Range", "npt=0.000-")]).await?;

        Ok(())
    }

    fn parse_www_authenticate(&mut self, headers: &str) {
        for line in headers.lines() {
            if line.starts_with("WWW-Authenticate: ") {
                let auth_header = &line["WWW-Authenticate: ".len()..];
                if auth_header.starts_with("Digest ") {
                    self.auth_method = AuthMethod::Digest;
                    let parts: HashMap<_, _> = auth_header["Digest ".len()..]
                        .split(',')
                        .filter_map(|part| {
                            let mut parts = part.trim().splitn(2, '=');
                            let key = parts.next()?.trim();
                            let value = parts.next()?.trim_matches('"').trim();
                            Some((key, value))
                        })
                        .collect();

                    if let Some(realm) = parts.get("realm") {
                        self.realm = Some(realm.to_string());
                    }
                    if let Some(nonce) = parts.get("nonce") {
                        self.nonce = Some(nonce.to_string());
                    }
                } else if auth_header.starts_with("Basic ") {
                    self.auth_method = AuthMethod::Basic;
                }
                break;
            }
        }
    }

    async fn send_request_with_auth(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)]
    ) -> Result<Vec<u8>> {
        let request = self.build_request(method, url, headers);
        log::debug!("Sending initial {} request", method);
        self.send_request_internal(&request).await?;
        
        let response = self.read_response_internal().await?;
        let status = self.parse_status(&response)?;
        log::debug!("{} response status: {}", method, status);

        if status == 401 {
            log::debug!("Got 401, analyzing WWW-Authenticate header");
            let (response_headers, _) = self.split_response(&response)?;
            self.parse_www_authenticate(response_headers);

            log::debug!("Attempting authentication with method: {:?}", self.auth_method);
            match self.auth_method {
                AuthMethod::Digest => {
                    let auth_request = self.build_authenticated_request_digest(method, url, headers)?;
                    self.send_request_internal(&auth_request).await?;
                    let auth_response = self.read_response_internal().await?;
                    let auth_status = self.parse_status(&auth_response)?;

                    if auth_status == 200 {
                        Ok(auth_response)
                    } else {
                        Err(VdkError::Protocol(format!("Authentication failed with status {}", auth_status)))
                    }
                }
                AuthMethod::Basic => {
                    let auth_request = self.build_authenticated_request_basic(method, url, headers)?;
                    self.send_request_internal(&auth_request).await?;
                    let auth_response = self.read_response_internal().await?;
                    let auth_status = self.parse_status(&auth_response)?;

                    if auth_status == 200 {
                        Ok(auth_response)
                    } else {
                        Err(VdkError::Protocol(format!("Authentication failed with status {}", auth_status)))
                    }
                }
                AuthMethod::None => {
                    Err(VdkError::Protocol("Authentication required but no credentials available".into()))
                }
            }
        } else if status != 200 {
            Err(VdkError::Protocol(format!("{} failed with status {}", method, status)))
        } else {
            Ok(response)
        }
    }

    async fn send_request_internal(&mut self, request: &str) -> Result<()> {
        let conn = self.connection.as_mut()
            .ok_or_else(|| VdkError::Protocol("Not connected".into()))?;
        
        conn.write_all(request.as_bytes()).await?;
        Ok(())
    }

    async fn read_response_internal(&mut self) -> Result<Vec<u8>> {
        let conn = self.connection.as_mut()
            .ok_or_else(|| VdkError::Protocol("Not connected".into()))?;

        conn.read_response().await
    }

    fn build_authenticated_request_digest(&mut self, method: &str, url: &str, headers: &[(&str, &str)]) -> Result<String> {
        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            log::debug!("Building digest authenticated request for user: {}", username);

            let realm = self.realm.as_deref().unwrap_or("RTSP Server");
            let nonce = self.nonce.as_deref().unwrap_or("none");

            let ha1 = md5_hash(&format!("{}:{}:{}", username, realm, password));
            let ha2 = md5_hash(&format!("{}:{}", method, url));
            let response = md5_hash(&format!("{}:{}:{}", ha1, nonce, ha2));

            let authorization_header = format!(
                r#"Digest username="{}", realm="{}", nonce="{}", uri="{}", response="{}""#,
                username, realm, nonce, url, response
            );

            let mut auth_headers: Vec<(&str, &str)> = vec![("Authorization", &authorization_header)];
            auth_headers.extend(headers);
            let request = self.build_request(method, url, &auth_headers);
            log::debug!("Built authenticated request with Digest headers");

            Ok(request)
        } else {
            log::debug!("No credentials available for authentication");
            Err(VdkError::Protocol("Authentication required but no credentials available".into()))
        }
    }

    fn build_authenticated_request_basic(&self, method: &str, url: &str, headers: &[(&str, &str)]) -> Result<String> {
        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            log::debug!("Building basic authenticated request for user: {}", username);

            let auth = format!("{}:{}", username, password);
            let auth_base64 = base64::engine::general_purpose::STANDARD.encode(auth.as_bytes());
            let auth_header = format!("Basic {}", auth_base64);

            let mut auth_headers: Vec<(&str, &str)> = vec![("Authorization", &auth_header)];
            auth_headers.extend(headers);
            let request = self.build_request(method, url, &auth_headers);
            log::debug!("Built basic authenticated request");

            Ok(request)
        } else {
            log::debug!("No credentials available for authentication");
            Err(VdkError::Protocol("Authentication required but no credentials available".into()))
        }
    }

    fn build_request(&self, method: &str, url: &str, headers: &[(&str, &str)]) -> String {
        let cseq = self.cseq.fetch_add(1, Ordering::SeqCst);

        let mut request = format!("{} {} RTSP/1.0\r\n", method, url);
        request.push_str(&format!("CSeq: {}\r\n", cseq));
        request.push_str("User-Agent: vdkio\r\n");

        for (name, value) in headers {
            request.push_str(&format!("{}: {}\r\n", name, value));
        }

        request.push_str("\r\n");
        log::debug!("Built request with method {} and {} headers", method, headers.len());
        request
    }

    fn parse_status(&self, response: &[u8]) -> Result<u32> {
        let response = String::from_utf8_lossy(response);
        let status_line = response.lines().next()
            .ok_or_else(|| VdkError::Protocol("Empty response".into()))?;

        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(VdkError::Protocol("Invalid status line".into()));
        }

        parts[1].parse::<u32>()
            .map_err(|_| VdkError::Protocol("Invalid status code".into()))
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
}

fn md5_hash(s: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}