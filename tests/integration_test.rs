#[cfg(test)]
mod tests {
    use vdkio::format::rtsp::RTSPClient;
    
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_rtsp_client_live() {
        // Replace with a valid RTSP URL for testing
        let url = "rtsp://example.com/stream";
        
        // Set a timeout for the entire test
        let result = timeout(Duration::from_secs(10), async {
            let mut client = RTSPClient::new(url)?;

            // Attempt to connect
            if let Err(e) = client.connect().await {
                println!("Connection failed: {}", e);
                return Err(e);
            }

            // Attempt to describe
            let sdp = match client.describe().await {
                Ok(sdp) => sdp,
                Err(e) => {
                    println!("Describe failed: {}", e);
                    return Err(e);
                }
            };

            // Attempt to set up available media streams
            for media in &sdp {
                if let Err(e) = client.setup(media).await {
                    println!("Setup failed for media type {}: {}", media.media_type, e);
                    // Continue to setup other streams even if one fails
                }
            }

            // Attempt to play
            if let Err(e) = client.play().await {
                println!("Play failed: {}", e);
                return Err(e);
            }

            println!("RTSP client test completed (limited checks)");
            Ok(())
        }).await;

        match result {
            Ok(Ok(())) => {}, // Test passed within timeout
            Ok(Err(e)) => panic!("RTSP client test failed: {}", e),
            Err(_) => panic!("RTSP client test timed out"), // Timeout occurred
        }
    }
}
