#[cfg(test)]
mod tests {
    use base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    use bytes::Bytes;
    use core::future::Future;
    use std::net::ToSocketAddrs;
    use std::path::PathBuf;
    use std::process::Command;
    use tokio::fs::File;
    use tokio::time;
    use tokio::time::{timeout, Duration}; 
    use vdkio::av;
    use vdkio::av::CodecData;
    use vdkio::av::Packet;
    use vdkio::error::{Result, VdkError};
    use vdkio::format::rtsp::{MediaStream, RTSPClient, TransportInfo};
    use vdkio::format::ts::{HLSSegmenter, TSMuxer};
    use vdkio::format::{Muxer, RTPPacket};
    use tokio::sync::mpsc;

    const TEST_NETWORK_CONNECTION_TIMEOUT: u64 = 5;
    const TEST_RTSP_CONNECT_TIMEOUT: u64 = 15;
    const TEST_RTSP_DESCRIBE_TIMEOUT: u64 = 60;
    const TEST_RTSP_SETUP_TIMEOUT: u64 = 30;
    const TEST_RTSP_PLAY_TIMEOUT: u64 = 30;
    const TEST_HLS_PLAYLIST_CHECK_TIMEOUT: u64 = 5;
    const TEST_PACKET_RECEIVE_TIMEOUT: u64 = 30;

    const RTSP_URL: &str = "rtsp://example.com:3000/cam/realmonitor?channel=1&subtype=0";    

    fn test_network_connection(host: &str, port: u16) -> bool {
        // Try basic TCP connection first
        if let Ok(addrs) = format!("{}:{}", host, port).to_socket_addrs() {
            if let Some(addr) = addrs.into_iter().next() {
                println!("Attempting TCP connection to {}...", addr);
                if std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(TEST_NETWORK_CONNECTION_TIMEOUT)).is_ok() {
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
                println!(
                    "Ping test {}",
                    if success { "successful" } else { "failed" }
                );
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
        // let rtsp_url = get_url_from_env(
        //     "TEST_RTSP_URL",
        //     "rtsp://example.com:3000/cam/realmonitor?channel=1&subtype=0",
        // );

        // Test network connectivity first
        println!("Testing network connectivity...");
        if !test_network_connection("45.122.123.142", 554) {
            return Err(VdkError::Codec(
                "Network connectivity test failed - RTSP server appears to be unreachable".into(),
            ));
        }

        // Create client outside of timeout to avoid including initialization in the timeout
        let mut client = RTSPClient::new(RTSP_URL)
            .map_err(|e| VdkError::Codec(format!("Failed to create client: {}", e)))?;

        // Connect with TEST_RTSP_CONNECT_TIMEOUT timeout
        println!("Attempting to connect...");
        with_timeout(TEST_RTSP_CONNECT_TIMEOUT, async {
            client
                .connect()
                .await
                .map_err(|e| VdkError::Codec(format!("Connect failed: {}", e)))
        })
        .await?;
        println!("Connected successfully");

        // Describe with TEST_RTSP_DESCRIBE_TIMEOUT timeout
        println!("Requesting stream description...");
        let sdp = with_timeout(TEST_RTSP_DESCRIBE_TIMEOUT, async {
            let result = client.describe().await;
            match &result {
                Ok(_) => println!("Describe request completed successfully"),
                Err(e) => println!("Describe request failed: {}", e),
            }
            result.map_err(|e| VdkError::Codec(format!("Describe failed: {}", e)))
        })
        .await?;
        println!("Received SDP with {} media streams", sdp.len());

        // Setup with TEST_RTSP_SETUP_TIMEOUT timeout per stream
        for media in &sdp {
            println!("Setting up media stream: {}", media.media_type);
            if let Err(e) = with_timeout(TEST_RTSP_SETUP_TIMEOUT, async {
                client
                    .setup(media)
                    .await
                    .map_err(|e| VdkError::Codec(format!("Setup failed: {}", e)))
            })
            .await
            {
                println!("Warning: Setup failed for {}: {}", media.media_type, e);
                // Continue with other streams
            }
        }
        println!("Media setup completed");

        // Play with TEST_RTSP_PLAY_TIMEOUT timeout
        println!("Starting playback...");
        with_timeout(TEST_RTSP_PLAY_TIMEOUT, async {
            client
                .play()
                .await
                .map_err(|e| VdkError::Codec(format!("Play failed: {}", e)))
        })
        .await?;
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

    // fn get_url_from_env(key: &str, default: &str) -> String {
    //     dotenv().ok();
    //     env::var(key).unwrap_or_else(|_| default.to_string())
    // }

    async fn check_directory(path: &PathBuf) -> Result<bool> {
        let mut interval = time::interval(Duration::from_millis(100));
        let start = time::Instant::now();
        let timeout = Duration::from_secs(TEST_HLS_PLAYLIST_CHECK_TIMEOUT);

        while start.elapsed() < timeout {
            interval.tick().await;

            if let Ok(mut entries) = tokio::fs::read_dir(path).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if entry.file_name() == "playlist.m3u8" {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    #[tokio::test]
    async fn test_rtsp_to_hls_pipeline() -> Result<()> {
        // Setup output directory
        let output_dir = PathBuf::from("test_output");
        if output_dir.exists() {
            tokio::fs::remove_dir_all(&output_dir).await?;
        }
        tokio::fs::create_dir_all(&output_dir).await?;
        println!("Created output directory at {}", output_dir.display());

        // Configure RTSP client
        let mut rtsp_client = RTSPClient::new(RTSP_URL)?;

        println!("Connecting to RTSP stream...");
        rtsp_client.connect().await?;

        // Get stream info
        let media = rtsp_client.describe().await?;
        println!("Found {} media streams", media.len());

        // Setup each media stream with TCP transport
        for (i, m) in media.iter().enumerate() {
            println!("Setting up {} stream", m.media_type);

            // Create base stream
            let (tx, _rx) = mpsc::channel(100);
            let stream = MediaStream::new(
                &m.media_type,
                &m.get_attribute("control")
                    .map(String::as_str)
                    .unwrap_or("*"),
                TransportInfo::new_rtp_avp((0, 0)), // Ports not used in TCP mode
                tx,
            ) 
            .with_tcp_transport((i as u16 * 2, i as u16 * 2 + 1));

            println!("Using transport: {}", stream.get_transport_str());
            rtsp_client.setup_with_stream(stream).await?;
        }

        // Configure HLS output
        let ts_output = File::create(output_dir.join("output.ts")).await?;
        let mut muxer = TSMuxer::new(ts_output);
        let segmenter =
            HLSSegmenter::new(&output_dir).with_segment_duration(Duration::from_secs(2));
        muxer = muxer.with_hls(segmenter);

        // Initialize muxer with stream info
        #[derive(Clone)]
        struct StreamInfo {
            codec_type: av::CodecType,
            width: Option<u32>,
            height: Option<u32>,
            extra_data: Option<Vec<u8>>,
        }

        impl CodecData for StreamInfo {
            fn codec_type(&self) -> av::CodecType {
                self.codec_type
            }
            fn width(&self) -> Option<u32> {
                self.width
            }
            fn height(&self) -> Option<u32> {
                self.height
            }
            fn extra_data(&self) -> Option<&[u8]> {
                self.extra_data.as_deref()
            }
        }

        let stream_codecs: Vec<Box<dyn av::CodecData>> = media
            .iter()
            .filter_map(|m| {
                let codec_type = match m.media_type.as_str() {
                    "video" => av::CodecType::H264,
                    "audio" => av::CodecType::AAC,
                    _ => return None,
                };

                // Extract SPS/PPS from fmtp if available
                let mut extra_data = None;
                if codec_type == av::CodecType::H264 {
                    if let Some(fmtp) = m.get_attribute("fmtp") {
                        if let Some(param) = fmtp
                            .split(';')
                            .find(|param| param.trim().starts_with("sprop-parameter-sets="))
                        {
                            if let Some(sets) = param.split('=').nth(1) {
                                let mut data = Vec::new();
                                for set in sets.split(',') {
                                    if let Ok(bytes) = BASE64_STANDARD.decode(set) {
                                        data.extend_from_slice(&[0, 0, 1]); // Add start code
                                        data.extend_from_slice(&bytes);
                                    }
                                }
                                if !data.is_empty() {
                                    extra_data = Some(data);
                                }
                            }
                        }
                    }
                }

                Some(Box::new(StreamInfo {
                    codec_type,
                    width: if m.media_type == "video" {
                        Some(1920)
                    } else {
                        None
                    },
                    height: if m.media_type == "video" {
                        Some(1080)
                    } else {
                        None
                    },
                    extra_data,
                }) as Box<dyn av::CodecData>)
            })
            .collect();

        muxer.write_header(&stream_codecs).await?;

        // Start playback
        println!("Starting playback...");
        rtsp_client.play().await?;

        // Read and process RTP packets
        if let Some(mut rx) = rtsp_client.get_packet_receiver() {
            let mut packets = 0;
            let mut last_write = time::Instant::now();

            while packets < 100 {
                // Process 100 packets
                match timeout(Duration::from_secs(TEST_PACKET_RECEIVE_TIMEOUT), rx.recv()).await {
                    Ok(Some(data)) => {
                        // Parse RTP packet
                        if let Ok(rtp) = RTPPacket::parse(&data) {
                            println!(
                                "Received RTP packet: seq={}, ts={}, pt={}, marker={}",
                                rtp.sequence_number, rtp.timestamp, rtp.payload_type, rtp.marker
                            );

                            // Convert RTP to AV packet
                            let stream_index = match rtp.payload_type {
                                96 => 0, // H.264
                                97 => 1, // AAC
                                _ => continue,
                            };

                            // For H.264, ensure we have NAL start codes
                            let mut data = Vec::with_capacity(rtp.payload.len() + 4);
                            if stream_index == 0 {
                                data.extend_from_slice(&[0, 0, 0, 1]); // Add H.264 start code
                            }
                            data.extend_from_slice(&rtp.payload);
                            let payload = Bytes::from(data);

                            let packet = Packet::new(payload)
                                .with_stream_index(stream_index)
                                .with_pts((rtp.timestamp as i64 / 90) as i64) // Convert 90kHz to ms
                                .with_key_flag(rtp.marker); // Use marker bit for keyframe

                            muxer.write_packet(&packet).await?;
                            packets += 1;
                            println!("Processed packet {}", packets);

                            // Write segments periodically
                            if last_write.elapsed() > Duration::from_secs(2) {
                                println!("Writing segment after {} packets", packets);
                                muxer.write_trailer().await?;
                                last_write = time::Instant::now();

                                // Check if playlist exists
                                if check_directory(&output_dir).await? {
                                    println!("Found playlist.m3u8!");
                                    break;
                                }
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        println!("Packet receive timeout");
                        muxer.write_trailer().await?;
                        break;
                    }
                }
            }
            println!("Processed {} packets", packets);

            // Final cleanup
            rtsp_client.teardown().await?;
            muxer.write_trailer().await?;

            // Wait a bit for filesystem operations to complete
            time::sleep(Duration::from_secs(1)).await;

            // Verify HLS output exists
            println!("Checking output at {}", output_dir.display());
            let playlist_exists = check_directory(&output_dir).await?;
            assert!(playlist_exists, "HLS playlist was not created");

            println!("RTSP to HLS conversion test completed successfully");
            Ok(())
        } else {
            Err(VdkError::Protocol("Failed to get packet receiver".into()))
        }
    }
}
