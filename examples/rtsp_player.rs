use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;
use vdkio::format::rtsp::RTSPClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create RTSP client
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "rtsp://example.com/stream".to_string());

    println!("Connecting to {}", url);
    let mut client = RTSPClient::new(&url)?;

    // Connect to server
    //client.connect().await?;
    //println!("Connected successfully");

    // Get stream information
    let sdp = client.describe().await?;
    println!("Received SDP description:");

    // Set up video stream if available
    if let Some(video) = sdp.iter().find(|m| m.media_type == "video") {
        println!("Setting up video stream:");
        println!("  Format: {}", video.format);
        println!("  Protocol: {}", video.protocol);
        if let Some(control) = video.get_attribute("control") {
            println!("  Control: {}", control);
        }
        //client.setup(video).await?;
        println!("Video stream setup complete");
    }

    // Set up audio stream if available
    if let Some(audio) = sdp.iter().find(|m| m.media_type == "audio") {
        println!("Setting up audio stream:");
        println!("  Format: {}", audio.format);
        println!("  Protocol: {}", audio.protocol);
        if let Some(control) = audio.get_attribute("control") {
            println!("  Control: {}", control);
        }
        //client.setup(audio).await?;
        println!("Audio stream setup complete");
    }

    // Get packet receiver for handling media data
    if let Some(mut rx) = client.get_packet_receiver() {
        println!("Starting packet receiver");

        // Spawn packet handling task
        tokio::spawn(async move {
            let mut video_packets = 0;
            let mut audio_packets = 0;
            let start_time = std::time::Instant::now();

            while let Some(packet) = rx.recv().await {
                // Basic RTP packet analysis
                if packet.len() >= 12 {
                    // Minimum RTP header size
                    let payload_type = packet[1] & 0x7f;
                    match payload_type {
                        96..=99 => {
                            // Common video payload types
                            video_packets += 1;
                        }
                        0..=95 => {
                            // Common audio payload types
                            audio_packets += 1;
                        }
                        _ => {}
                    }

                    // Print statistics every second
                    let elapsed = start_time.elapsed().as_secs();
                    if elapsed > 0 && elapsed % 1 == 0 {
                        println!("Statistics after {} seconds:", elapsed);
                        println!(
                            "  Video packets: {} ({} p/s)",
                            video_packets,
                            video_packets as f64 / elapsed as f64
                        );
                        println!(
                            "  Audio packets: {} ({} p/s)",
                            audio_packets,
                            audio_packets as f64 / elapsed as f64
                        );
                    }
                }
            }
        });
    }

    // Start playback
    println!("Starting playback");
    client.play().await?;
    println!("Playback started");

    // Keep program running to receive packets
    println!("Receiving stream for 30 seconds...");
    sleep(Duration::from_secs(30)).await;
    println!("Done");

    Ok(())
}
