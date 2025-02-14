#[cfg(test)]
mod tests {
    use vdkio::format::rtsp::RTSPClient;
    use vdkio::error::{Result, VdkError};
    use tokio::time::{timeout, Duration};
    use core::future::Future;
    use std::process::Command;
    use std::net::ToSocketAddrs;

    fn test_network_connection(host: &str, port: u16) -> bool {
        // Try basic TCP connection first
        if let Ok(addrs) = format!("{}:{}", host, port).to_socket_addrs() {
            if let Some(addr) = addrs.into_iter().next() {
                println!("Attempting TCP connection to {}...", addr);
                if std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(5)).is_ok() {
                    println!("TCP connection successful");
                    return true;
                }
            }
        }
        println!("TCP connection failed");

        // Try ping as fallback
        let output = if cfg!(target_os = "windows") {
            Command::new("ping")
                .args(["-n", "1", "-w", "5000", host])
                .output()
        } else {
            Command::new("ping")
                .args(["-c", "1", "-W", "5", host])
                .output()
        };

        match output {
            Ok(output) => {
                let success = output.status.success();
                println!("Ping test {}", if success { "successful" } else { "failed" });
                success
            }
            Err(e) => {
                println!("Ping test error: {}", e);
                false
            }
        }
    }

    #[tokio::test]
    async fn test_rtsp_client_live() -> Result<()> {
        let url = "rtsp://example.com/stream";
        
        // Test network connectivity first
        println!("Testing network connectivity...");
        if !test_network_connection("45.122.123.142", 554) {
            return Err(VdkError::Codec("Network connectivity test failed - RTSP server appears to be unreachable".into()));
        }

        // Create client outside of timeout to avoid including initialization in the timeout
        let mut client = RTSPClient::new(url).map_err(|e| VdkError::Codec(format!("Failed to create client: {}", e)))?;

        // Connect with 30s timeout
        println!("Attempting to connect...");
        with_timeout(30, async {
            client.connect().await.map_err(|e| VdkError::Codec(format!("Connect failed: {}", e)))
        }).await?;
        println!("Connected successfully");

        // Describe with 120s timeout
        println!("Requesting stream description...");
        let sdp = with_timeout(120, async {
            let result = client.describe().await;
            match &result {
                Ok(_) => println!("Describe request completed successfully"),
                Err(e) => println!("Describe request failed: {}", e),
            }
            result.map_err(|e| VdkError::Codec(format!("Describe failed: {}", e)))
        }).await?;
        println!("Received SDP with {} media streams", sdp.len());

        // Setup with 60s timeout per stream
        for media in &sdp {
            println!("Setting up media stream: {}", media.media_type);
            if let Err(e) = with_timeout(60, async {
                client.setup(media).await.map_err(|e| VdkError::Codec(format!("Setup failed: {}", e)))
            }).await
            {
                println!("Warning: Setup failed for {}: {}", media.media_type, e);
                // Continue with other streams
            }
        }
        println!("Media setup completed");

        // Play with 60s timeout
        println!("Starting playback...");
        with_timeout(60, async {
            client.play().await.map_err(|e| VdkError::Codec(format!("Play failed: {}", e)))
        }).await?;
        println!("Playback started successfully");

        // Test passed
        println!("RTSP client test completed successfully");
        Ok(())
    }

    async fn with_timeout<F, T>(duration: u64, operation: F) -> std::result::Result<T, VdkError>
    where
        F: Future<Output = std::result::Result<T, VdkError>>,
    {
        match timeout(Duration::from_secs(duration), operation).await {
            Ok(result) => result,
            Err(_) => Err(VdkError::Codec("Operation timed out".into())),
        }
    }
}
