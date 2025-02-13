use crate::Result;
use crate::VdkError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Debug)]
pub struct RTSPConnection {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl RTSPConnection {
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| VdkError::Protocol(format!("Failed to connect to {}: {}", addr, e)))?;
        
        stream.set_nodelay(true)?;

        Ok(Self {
            stream,
            buffer: Vec::with_capacity(4096),
        })
    }

    pub async fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.stream.write_all(data).await?;
        self.stream.flush().await?;
        Ok(())
    }

    pub async fn read_response(&mut self) -> Result<Vec<u8>> {
        self.buffer.clear();
        let mut temp_buf = [0; 4096];
        
        loop {
            match self.stream.read(&mut temp_buf).await {
                Ok(0) => {
                    if self.buffer.is_empty() {
                        return Err(VdkError::Protocol("Connection closed by peer".into()));
                    }
                    break;
                }
                Ok(n) => {
                    self.buffer.extend_from_slice(&temp_buf[..n]);
                    if self.is_response_complete() {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(self.buffer.clone())
    }

    fn is_response_complete(&self) -> bool {
        if self.buffer.len() < 4 {
            return false;
        }

        // Look for double CRLF which marks end of headers
        for i in 0..self.buffer.len() - 3 {
            if &self.buffer[i..i + 4] == b"\r\n\r\n" {
                // If we have Content-Length, verify we have the full body
                if let Some(content_length) = self.get_content_length() {
                    return self.buffer.len() >= i + 4 + content_length;
                }
                return true;
            }
        }
        false
    }

    fn get_content_length(&self) -> Option<usize> {
        let content = std::str::from_utf8(&self.buffer).ok()?;
        for line in content.lines() {
            if line.to_lowercase().starts_with("content-length:") {
                return line[15..].trim().parse().ok();
            }
        }
        None
    }
}

impl Drop for RTSPConnection {
    fn drop(&mut self) {
        // Best effort to close the connection gracefully
        let _ = self.stream.shutdown();
    }
}
