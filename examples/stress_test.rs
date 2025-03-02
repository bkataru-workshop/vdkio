use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::sync::mpsc;
use tokio::time;
use vdkio::av::{CodecData, CodecDataExt, CodecType, Packet};
use vdkio::config;
use vdkio::error::Result;
use vdkio::format::rtsp::{MediaStream, RTSPClient, RTSPSetupOptions, TransportInfo};
use vdkio::format::ts::{HLSSegmenter, TSMuxer};
use vdkio::format::Muxer;

// Remove hardcoded RTSP_URL
const STRESS_TEST_DURATION: Duration = Duration::from_secs(300); // 5 minutes
const STATS_INTERVAL: Duration = Duration::from_secs(10);
const SEGMENT_DURATION_MS: i64 = 2000; // 2 seconds in milliseconds

#[derive(Clone)]
struct Stats {
    packets_processed: Arc<AtomicU64>,
    bytes_processed: Arc<AtomicU64>,
    segments_created: Arc<AtomicU64>,
    errors_encountered: Arc<AtomicU64>,
}

impl Stats {
    fn new() -> Self {
        Self {
            packets_processed: Arc::new(AtomicU64::new(0)),
            bytes_processed: Arc::new(AtomicU64::new(0)),
            segments_created: Arc::new(AtomicU64::new(0)),
            errors_encountered: Arc::new(AtomicU64::new(0)),
        }
    }

    fn print_stats(&self, elapsed: Duration) {
        let packets = self.packets_processed.load(Ordering::Relaxed);
        let bytes = self.bytes_processed.load(Ordering::Relaxed);
        let segments = self.segments_created.load(Ordering::Relaxed);
        let errors = self.errors_encountered.load(Ordering::Relaxed);

        let elapsed_secs = elapsed.as_secs_f64();
        let packets_per_sec = packets as f64 / elapsed_secs;
        let mbps = (bytes as f64 * 8.0 / 1_000_000.0) / elapsed_secs;

        println!("=== Performance Statistics ===");
        println!("Time elapsed: {:.2?}", elapsed);
        println!("Packets processed: {} ({:.2} packets/sec)", packets, packets_per_sec);
        println!("Data throughput: {:.2} Mbps", mbps);
        println!("Segments created: {}", segments);
        println!("Errors encountered: {}", errors);
        println!("==========================");
    }
}

#[derive(Clone)]
struct StreamInfo {
    codec_type: CodecType,
    width: Option<u32>,
    height: Option<u32>,
}

impl CodecData for StreamInfo {
    fn codec_type(&self) -> CodecType {
        self.codec_type
    }
    fn width(&self) -> Option<u32> {
        self.width
    }
    fn height(&self) -> Option<u32> {
        self.height
    }
    fn extra_data(&self) -> Option<&[u8]> {
        None
    }
}

// StreamInfo implements Clone + CodecData, so it gets CodecDataExt through blanket impl

async fn run_stress_test(output_dir: &Path, stats: Stats) -> Result<()> {
    // Get RTSP URL from config
    let url = config::get_rtsp_url();
    
    // Ensure output directory exists
    tokio::fs::create_dir_all(output_dir).await?;

    // Setup RTSP client with options
    println!("Connecting to RTSP stream: {}", url);
    let mut client = RTSPClient::new(&url)?;
    let options = RTSPSetupOptions::new()
        .with_video(true)
        .with_audio(true);

    // Connect and get stream info
    client.connect().await?;
    let media = client.describe().await?;
    println!("Found {} media streams", media.len());

    // Setup multi-bitrate variants
    let variants = vec![
        ("high", 2_000_000, (1280, 720)),   // 720p
        ("medium", 1_000_000, (854, 480)),  // 480p
        ("low", 500_000, (640, 360)),      // 360p
    ];

    // Create HLS segmenters and muxers for each variant
    let mut muxers = Vec::new();
    for (name, _bitrate, resolution) in &variants {
        let variant_dir = output_dir.join(name);
        tokio::fs::create_dir_all(&variant_dir).await?;

        let ts_output = File::create(variant_dir.join("output.ts")).await?;
        let mut muxer = TSMuxer::new(ts_output);
        
        let segmenter = HLSSegmenter::new(&variant_dir)
            .with_segment_duration(Duration::from_millis(SEGMENT_DURATION_MS as u64));
        muxer = muxer.with_hls(segmenter);

        // Initialize with codec info
        let stream_info: Vec<Box<dyn CodecDataExt>> = vec![
            Box::new(StreamInfo {
                codec_type: CodecType::H264,
                width: Some(resolution.0),
                height: Some(resolution.1),
            }),
            Box::new(StreamInfo {
                codec_type: CodecType::AAC,
                width: None,
                height: None,
            }),
        ];

        muxer.write_header(&stream_info).await?;
        muxers.push(muxer);
    }

    // Setup media streams with options
    for m in &media {
        if (m.media_type == "video" && options.enable_video)
            || (m.media_type == "audio" && options.enable_audio)
        {
            let (tx, _rx) = mpsc::channel(100);
            let stream = MediaStream::new(
                &m.media_type,
                "trackID=0",
                TransportInfo::new_rtp_avp((0, 0)),
                tx,
            ).with_tcp_transport((0, 1));

            if let Err(e) = client.setup_with_stream(stream).await {
                stats.errors_encountered.fetch_add(1, Ordering::Relaxed);
                println!("Error setting up stream: {}", e);
                continue;
            }
        }
    }

    // Start playback
    println!("Starting playback...");
    client.play().await?;

    let start_time = Instant::now();
    let stats_clone = stats.clone();

    // Spawn stats printer
    tokio::spawn(async move {
        let mut interval = time::interval(STATS_INTERVAL);
        while start_time.elapsed() < STRESS_TEST_DURATION {
            interval.tick().await;
            stats_clone.print_stats(start_time.elapsed());
        }
    });

    if let Some(mut rx) = client.get_packet_receiver() {
        while start_time.elapsed() < STRESS_TEST_DURATION {
            match time::timeout(Duration::from_secs(5), rx.recv()).await {
                Ok(Some(data)) => {
                    stats.packets_processed.fetch_add(1, Ordering::Relaxed);
                    stats.bytes_processed.fetch_add(data.len() as u64, Ordering::Relaxed);

                    // Create packet with current timestamp
                    let pts = start_time.elapsed().as_millis() as i64;
                    let packet = Packet::new(data)
                        .with_stream_index(0)
                        .with_pts(pts);

                    // Process packet for each variant
                    for muxer in &mut muxers {
                        if let Err(e) = muxer.write_packet(&packet).await {
                            stats.errors_encountered.fetch_add(1, Ordering::Relaxed);
                            println!("Error writing packet: {}", e);
                            continue;
                        }

                        // Create new segment every SEGMENT_DURATION_MS
                        if pts % SEGMENT_DURATION_MS == 0 {
                            if let Err(e) = muxer.write_trailer().await {
                                stats.errors_encountered.fetch_add(1, Ordering::Relaxed);
                                println!("Error writing trailer: {}", e);
                            } else {
                                stats.segments_created.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
                Ok(None) => {
                    println!("Stream ended");
                    break;
                }
                Err(_) => {
                    stats.errors_encountered.fetch_add(1, Ordering::Relaxed);
                    println!("Timeout receiving packet, attempting reconnect...");
                    
                    if let Err(e) = client.reconnect().await {
                        stats.errors_encountered.fetch_add(1, Ordering::Relaxed);
                        println!("Reconnection failed: {}", e);
                        break;
                    }

                    if let Err(e) = client.play().await {
                        stats.errors_encountered.fetch_add(1, Ordering::Relaxed);
                        println!("Failed to restart playback: {}", e);
                        break;
                    }

                    if let Some(new_rx) = client.get_packet_receiver() {
                        rx = new_rx;
                    } else {
                        println!("Failed to get new packet receiver");
                        break;
                    }
                }
            }
        }
    }

    // Final stats
    println!("\nFinal Statistics:");
    stats.print_stats(start_time.elapsed());

    // Cleanup
    client.teardown().await?;
    for muxer in &mut muxers {
        muxer.write_trailer().await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let output_dir = Path::new("stress_test_output");
    let stats = Stats::new();
    
    println!("Starting stress test...");
    println!("Duration: {:?}", STRESS_TEST_DURATION);
    println!("Output directory: {}", output_dir.display());
    
    if let Err(e) = run_stress_test(output_dir, stats).await {
        println!("Stress test failed: {}", e);
        std::process::exit(1);
    }
    
    println!("Stress test completed successfully");
    Ok(())
}