use vdkio::prelude::*;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "rtsp://example.com/stream".to_string());

    println!("Connecting to RTSP stream at: {}", url);

    // Create RTSP client
    let mut client = RTSPClient::new(&url)?;
    
    // Connect to the server
    client.connect().await?;
    println!("Connected successfully");

    // Get stream information
    let sdp = client.describe().await?;
    println!("\nStream information:");
    println!("Session name: {}", sdp.session_name.as_deref().unwrap_or("unnamed"));
    
    // Initialize parsers
    let mut h264_parser = H264Parser::new();
    let mut video_port: Option<u16> = None;

    // Set up video stream if available
    if let Some(video) = sdp.get_media("video") {
        println!("\nFound video stream:");
        println!("  Format: {}", video.format);
        println!("  Protocol: {}", video.protocol);
        println!("  Port: {}", video.port);
        
        for (key, value) in &video.attributes {
            println!("  {}: {}", key, value);
        }
        
        client.setup("video").await?;
        video_port = Some(video.port);
    }

    // Set up audio stream if available
    if let Some(audio) = sdp.get_media("audio") {
        println!("\nFound audio stream:");
        println!("  Format: {}", audio.format);
        println!("  Protocol: {}", audio.protocol);
        println!("  Port: {}", audio.port);
        
        for (key, value) in &audio.attributes {
            println!("  {}: {}", key, value);
        }
        
        client.setup("audio").await?;
    }

    // Start playback
    println!("\nStarting playback...");
    client.play().await?;

    // --- RTP Packet Processing ---
    if let Some(rtp_port) = video_port {
        println!("\nSetting up RTP receiver on port {}", rtp_port);
        
        // Create a UDP socket for receiving RTP packets
        // Bind to 0.0.0.0 to receive packets from any interface
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", rtp_port)).await?;
        println!("UDP socket bound successfully");

        let mut packet_count = 0;
        let mut rtp_buffer = vec![0u8; 4096];
        let start_time = tokio::time::Instant::now();
        let duration = Duration::from_secs(10);

        println!("\nReceiving RTP packets for {} seconds...", duration.as_secs());

        while tokio::time::Instant::now().duration_since(start_time) < duration {
            tokio::select! {
                Ok((len, _addr)) = socket.recv_from(&mut rtp_buffer) => {
                    if len < 12 {
                        println!("Received incomplete RTP packet");
                        continue;
                    }

                    packet_count += 1;

                    // Parse basic RTP header
                    let version = (rtp_buffer[0] >> 6) & 0x03;
                    let padding = (rtp_buffer[0] >> 5) & 0x01;
                    let extension = (rtp_buffer[0] >> 4) & 0x01;
                    let csrc_count = rtp_buffer[0] & 0x0f;
                    let marker = (rtp_buffer[1] >> 7) & 0x01;
                    let payload_type = rtp_buffer[1] & 0x7f;
                    let sequence_number = ((rtp_buffer[2] as u16) << 8) | (rtp_buffer[3] as u16);
                    let timestamp = ((rtp_buffer[4] as u32) << 24) | 
                                  ((rtp_buffer[5] as u32) << 16) |
                                  ((rtp_buffer[6] as u32) << 8) |
                                  (rtp_buffer[7] as u32);

                    println!("\nRTP Packet #{}", packet_count);
                    println!("  Version: {}", version);
                    println!("  Padding: {}", padding);
                    println!("  Extension: {}", extension);
                    println!("  CSRC Count: {}", csrc_count);
                    println!("  Marker: {}", marker);
                    println!("  Payload Type: {}", payload_type);
                    println!("  Sequence Number: {}", sequence_number);
                    println!("  Timestamp: {}", timestamp);

                    // Calculate header size (12 bytes + 4 bytes per CSRC)
                    let header_size = 12 + (csrc_count as usize * 4);
                    if len > header_size {
                        let payload = &rtp_buffer[header_size..len];
                        
                        // For H.264 (payload type 96 by convention)
                        if payload_type == 96 {
                            match h264_parser.parse_nalu(payload) {
                                Ok(nalu) => {
                                    println!("  H.264 NALU Type: {}", nalu.nal_type);
                                    if nalu.is_keyframe() {
                                        println!("  --> Keyframe detected!");
                                    }
                                },
                                Err(e) => println!("  Failed to parse H.264 NALU: {}", e),
                            }
                        }
                    }
                }
                _ = sleep(Duration::from_millis(100)) => {
                    // Small delay to prevent busy-waiting
                }
            }
        }

        println!("\nReceived {} RTP packets in {} seconds", 
            packet_count, 
            duration.as_secs());
    }
    
    println!("\nStream processing complete");
    Ok(())
}
