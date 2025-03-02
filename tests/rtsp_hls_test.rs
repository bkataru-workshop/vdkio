use bytes::Bytes;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs::File;
use tokio::time::timeout;
use vdkio::av::{CodecData, CodecDataExt, CodecType, Packet};
use vdkio::config;
use vdkio::error::{Result, VdkError};
use vdkio::format::rtsp::{MediaStream, RTSPClient, TransportInfo};
use vdkio::format::ts::{HLSSegmenter, TSMuxer};
use vdkio::format::{Muxer, RTPPacket};

const SEGMENT_DURATION: Duration = Duration::from_secs(2);
const TEST_TIMEOUT: Duration = Duration::from_secs(30);

// Get RTSP URL from config instead of hardcoding it
fn get_test_rtsp_url() -> String {
    config::get_rtsp_url()
}

#[tokio::test]
async fn test_rtsp_reconnection() -> Result<()> {
    let mut client = RTSPClient::new(&get_test_rtsp_url())?;
    
    // Initial connection
    client.connect().await?;
    let media = client.describe().await?;
    
    // Setup streams
    for m in &media {
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        let stream = MediaStream::new(
            &m.media_type,
            "trackID=0",
            TransportInfo::new_rtp_avp((0, 0)),
            tx,
        ).with_tcp_transport((0, 1));
        client.setup_with_stream(stream).await?;
    }
    
    // Start playback
    client.play().await?;
    
    // Simulate network interruption after receiving some packets
    if let Some(mut rx) = client.get_packet_receiver() {
        let mut packets = 0;
        while packets < 10 {
            if timeout(Duration::from_secs(5), rx.recv()).await.is_ok() {
                packets += 1;
            }
        }
        
        // Force disconnect
        client.teardown().await?;
        
        // Attempt reconnection
        assert!(client.reconnect().await?, "Reconnection failed");
        
        // Verify stream is working after reconnect
        if let Some(mut rx) = client.get_packet_receiver() {
            let received = timeout(Duration::from_secs(5), rx.recv()).await.is_ok();
            assert!(received, "No packets received after reconnection");
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_ts_segment_validation() -> Result<()> {
    let output_dir = PathBuf::from("test_output/ts_validation");
    tokio::fs::create_dir_all(&output_dir).await?;
    
    let mut client = RTSPClient::new(&get_test_rtsp_url())?;
    client.connect().await?;
    
    let media = client.describe().await?;
    let mut video_stream_index = None;
    
    // Setup streams and track codec info
    for (i, m) in media.iter().enumerate() {
        if m.media_type == "video" {
            video_stream_index = Some(i);
            let (tx, _rx) = tokio::sync::mpsc::channel(100);
            let stream = MediaStream::new(
                "video",
                "trackID=0",
                TransportInfo::new_rtp_avp((0, 0)),
                tx,
            ).with_tcp_transport((0, 1));
            client.setup_with_stream(stream).await?;
        }
    }
    
    let video_index = video_stream_index.ok_or_else(|| {
        VdkError::Protocol("No video stream found".into())
    })?;
    
    // Configure TS muxer and HLS segmenter
    let ts_output = File::create(output_dir.join("output.ts")).await?;
    let mut muxer = TSMuxer::new(ts_output);
    let segmenter = HLSSegmenter::new(&output_dir)
        .with_segment_duration(SEGMENT_DURATION);
    muxer = muxer.with_hls(segmenter);
    
    // Write header with codec info
    let stream_info: Vec<Box<dyn CodecDataExt>> = vec![Box::new(StreamInfo {
        codec_type: CodecType::H264,
        width: Some(1920),
        height: Some(1080),
        extra_data: None,
    })];
    
    muxer.write_header(&stream_info).await?;
    
    // Start playback and collect segments
    client.play().await?;
    
    if let Some(mut rx) = client.get_packet_receiver() {
        let mut segment_count = 0;
        let mut last_pcr = 0i64;
        let mut last_segment_time = 0i64;
        
        while segment_count < 3 {
            match timeout(Duration::from_secs(5), rx.recv()).await {
                Ok(Some(data)) => {
                    if let Ok(rtp) = RTPPacket::parse(&data) {
                        let pts = (rtp.timestamp as i64 / 90) as i64;
                        
                        // Create packet
                        let packet = Packet::new(Bytes::from(data))
                            .with_stream_index(video_index)
                            .with_pts(pts)
                            .with_key_flag(rtp.marker);
                            
                        // Write packet and check timing
                        muxer.write_packet(&packet).await?;
                        
                        // Validate PCR timing
                        if pts > last_pcr {
                            let pcr_delta = pts - last_pcr;
                            assert!(pcr_delta <= 100, "PCR jump too large");
                            last_pcr = pts;
                        }
                        
                        // Check segment duration
                        if pts - last_segment_time >= SEGMENT_DURATION.as_millis() as i64 {
                            muxer.write_trailer().await?;
                            segment_count += 1;
                            last_segment_time = pts;
                            
                            // Validate segment exists
                            let segment_path = output_dir.join(format!("segment_{}.ts", segment_count));
                            assert!(segment_path.exists(), "Segment file not created");
                            
                            // Basic TS packet validation
                            let segment_data = tokio::fs::read(&segment_path).await?;
                            validate_ts_packet(&segment_data)?;
                        }
                    }
                }
                _ => break,
            }
        }
        
        // Verify playlist creation
        let playlist_path = output_dir.join("playlist.m3u8");
        assert!(playlist_path.exists(), "Playlist file not created");
        
        // Validate playlist format
        let playlist_content = tokio::fs::read_to_string(playlist_path).await?;
        validate_m3u8_playlist(&playlist_content)?;
    }
    
    Ok(())
}

fn validate_ts_packet(data: &[u8]) -> Result<()> {
    if data.len() < 188 {
        return Err(VdkError::Codec("Invalid TS packet size".into()));
    }
    
    // Check sync byte
    if data[0] != 0x47 {
        return Err(VdkError::Codec("Invalid sync byte".into()));
    }
    
    // Basic PAT validation
    let has_pat = data.windows(188).any(|packet| {
        packet[0] == 0x47 && packet[1] & 0x40 != 0 && packet[3] & 0x10 != 0
    });
    
    if !has_pat {
        return Err(VdkError::Codec("Missing PAT".into()));
    }
    
    Ok(())
}

fn validate_m3u8_playlist(content: &str) -> Result<()> {
    // Basic M3U8 format validation
    if !content.starts_with("#EXTM3U") {
        return Err(VdkError::Protocol("Invalid playlist header".into()));
    }
    
    // Check for required tags
    if !content.contains("#EXT-X-VERSION") {
        return Err(VdkError::Protocol("Missing version tag".into()));
    }
    
    if !content.contains("#EXT-X-TARGETDURATION") {
        return Err(VdkError::Protocol("Missing target duration".into()));
    }
    
    // Validate segment entries
    let mut has_segments = false;
    for line in content.lines() {
        if line.starts_with("#EXTINF:") {
            has_segments = true;
            // Validate segment duration format
            if !line.contains(",") {
                return Err(VdkError::Protocol("Invalid segment info".into()));
            }
        }
    }
    
    if !has_segments {
        return Err(VdkError::Protocol("No segments found".into()));
    }
    
    Ok(())
}

#[derive(Debug, Clone)]
struct StreamInfo {
    codec_type: CodecType,
    width: Option<u32>,
    height: Option<u32>,
    extra_data: Option<Vec<u8>>,
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
        self.extra_data.as_deref()
    }
}

// StreamInfo implements Clone + CodecData, so it gets CodecDataExt through blanket impl

#[tokio::test]
async fn test_rtsp_error_handling() -> Result<()> {
    // Test invalid URL
    let result = RTSPClient::new("rtsp://invalid.example.com:1234/stream");
    assert!(result.is_ok(), "Client creation should succeed with invalid URL");
    
    let mut client = result?;
    let connect_result = client.connect().await;
    assert!(connect_result.is_err(), "Connection should fail for invalid URL");
    
    // Test authentication failure
    let mut client = RTSPClient::new("rtsp://example.com:3000/protected/stream")?;
    let auth_result = client.connect().await;
    assert!(auth_result.is_err(), "Should fail without credentials");
    
    // Test invalid stream setup
    let mut client = RTSPClient::new(&get_test_rtsp_url())?;
    client.connect().await?;
    
    let (tx, _rx) = tokio::sync::mpsc::channel(100);
    let invalid_stream = MediaStream::new(
        "invalid",
        "trackID=999",
        TransportInfo::new_rtp_avp((0, 0)),
        tx,
    );
    
    let setup_result = client.setup_with_stream(invalid_stream).await;
    assert!(setup_result.is_err(), "Setup should fail for invalid stream");
    
    Ok(())
}

#[tokio::test]
async fn test_performance() -> Result<()> {
    let start_time = std::time::Instant::now();
    let packet_count = 1000;
    let mut processed = 0;
    
    let mut client = RTSPClient::new(&get_test_rtsp_url())?;
    client.connect().await?;
    
    let media = client.describe().await?;
    for m in &media {
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        let stream = MediaStream::new(
            &m.media_type,
            "trackID=0",
            TransportInfo::new_rtp_avp((0, 0)),
            tx,
        ).with_tcp_transport((0, 1));
        client.setup_with_stream(stream).await?;
    }
    
    client.play().await?;
    
    if let Some(mut rx) = client.get_packet_receiver() {
        while processed < packet_count {
            if let Ok(Some(_)) = timeout(Duration::from_secs(1), rx.recv()).await {
                processed += 1;
            }
        }
    }
    
    let elapsed = start_time.elapsed();
    let packets_per_second = processed as f64 / elapsed.as_secs_f64();
    
    println!("Performance metrics:");
    println!("Total packets processed: {}", processed);
    println!("Total time: {:.2?}", elapsed);
    println!("Packets per second: {:.2}", packets_per_second);
    
    // Basic performance assertions
    assert!(packets_per_second > 10.0, "Performance below minimum threshold");
    
    Ok(())
}