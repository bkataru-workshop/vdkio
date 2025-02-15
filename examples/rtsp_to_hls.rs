use vdkio::format::rtsp::{RTSPClient, RTSPSetupOptions};
use vdkio::format::ts::{HLSSegmenter, TSMuxer, HLSVariant};
use vdkio::av::{CodecType, Packet};
use vdkio::format::Muxer;
use vdkio::av::transcode::{Transcoder, TranscodeOptions, StreamCodecData, VideoEncoder, VideoDecoder};
use vdkio::codec::h264::transcode::create_transcoder_for_resolution;
use vdkio::error::VdkError;
use std::time::Duration;
use std::path::Path;
use bytes::Bytes;
use tokio::fs::File;
use tokio::time;

#[allow(dead_code)]
struct HLSVariantConfig {
    name: String,
    bandwidth: u32,
    resolution: (u32, u32),
    fps: u32,
    segmenter: HLSSegmenter,
    muxer: TSMuxer<File>,
    transcoder: Transcoder,
}

fn create_codec_factory(width: u32, height: u32, bitrate: u32, fps: u32) -> Box<dyn Fn(&StreamCodecData) -> Result<(Box<dyn VideoEncoder>, Box<dyn VideoDecoder>), VdkError> + Send + Sync> {
    Box::new(create_transcoder_for_resolution(width, height, bitrate, fps))
}

async fn setup_variant(
    name: &str,
    bandwidth: u32,
    resolution: (u32, u32),
    fps: u32,
    output_dir: &Path,
    codecs: &[StreamCodecData],
) -> Result<HLSVariantConfig, Box<dyn std::error::Error>> {
    let variant = HLSVariant {
        name: name.to_string(),
        bandwidth,
        resolution: Some(resolution),
        codecs: "avc1.64001f,mp4a.40.2".to_string(), // H.264 High Profile + AAC-LC
    };

    let segmenter = HLSSegmenter::new(output_dir)
        .with_segment_duration(Duration::from_secs(2))
        .with_max_segments(5)
        .with_variant(variant);

    let ts_output = File::create(output_dir.join(format!("{}.ts", name))).await?;
    let mut muxer = TSMuxer::new(ts_output);

    // Create transcoding options based on target resolution
    let options = TranscodeOptions {
        find_video_codec: Some(create_codec_factory(resolution.0, resolution.1, bandwidth, fps)),
        find_audio_codec: None, // Pass through audio for now
    };

    let transcoder = Transcoder::new(codecs.to_vec(), options)?;
    
    // Initialize muxer with transcoded streams info
    let codec_boxes: Vec<Box<dyn vdkio::av::CodecData>> = transcoder.streams().iter()
        .map(|s| Box::new(s.clone()) as Box<dyn vdkio::av::CodecData>)
        .collect();
    muxer.write_header(&codec_boxes).await?;

    Ok(HLSVariantConfig {
        name: name.to_string(),
        bandwidth,
        resolution,
        fps,
        segmenter,
        muxer,
        transcoder,
    })
}

async fn setup_rtsp_client(url: &str, options: RTSPSetupOptions) -> Result<(RTSPClient, Vec<StreamCodecData>), Box<dyn std::error::Error>> {
    let mut client = RTSPClient::new(url)?;
    
    // Attempt initial connection
    if let Err(e) = client.connect().await {
        println!("Initial connection failed: {}", e);
        if !client.reconnect().await? {
            return Err(e.into());
        }
    }
    
    // Get stream information
    let media_descriptions = client.describe().await?;
    let mut codecs = Vec::new();
    
    // Setup each media stream
    for media in &media_descriptions {
        if (media.media_type == "video" && options.enable_video) ||
           (media.media_type == "audio" && options.enable_audio) {
            client.setup(media).await?;
            
            let codec_type = match &media.media_type[..] {
                "video" => CodecType::H264,
                "audio" => CodecType::AAC,
                _ => continue,
            };

            let (width, height) = if media.media_type == "video" {
                (Some(1920), Some(1080)) // Default to 1080p for original stream
            } else {
                (None, None)
            };

            codecs.push(StreamCodecData {
                codec_type,
                width,
                height,
                extra_data: None,
            });
        }
    }
    
    Ok((client, codecs))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration
    let rtsp_url = std::env::var("RTSP_URL")
        .expect("RTSP_URL environment variable must be set");
    let hls_output_dir = Path::new("output/hls");
    
    // Create output directory
    tokio::fs::create_dir_all(hls_output_dir).await?;

    // Setup RTSP client with options
    let setup_options = RTSPSetupOptions::new()
        .with_video(true)
        .with_audio(true);
    
    let (mut client, codecs) = setup_rtsp_client(&rtsp_url, setup_options).await?;

    // Create variant configs with different resolutions and bitrates
    let variants = vec![
        ("high", 2_000_000, (1280, 720), 30),    // 720p @ 30fps
        ("medium", 1_000_000, (854, 480), 30),   // 480p @ 30fps
        ("low", 500_000, (640, 360), 30),        // 360p @ 30fps
    ];

    let mut variant_configs = Vec::new();
    for (name, bandwidth, resolution, fps) in variants {
        let config = setup_variant(name, bandwidth, resolution, fps, hls_output_dir, &codecs).await?;
        variant_configs.push(config);
    }

    // Create master playlist
    let master_playlist_path = hls_output_dir.join("playlist.m3u8");
    let mut master_playlist_file = File::create(&master_playlist_path).await?;

    // Write initial master playlist
    variant_configs[0].segmenter.write_master_playlist(&mut master_playlist_file).await?;
    
    // Start playback
    client.play().await?;
    
    println!("Starting RTSP to multi-bitrate HLS conversion...");
    println!("Writing segments to: {}", hls_output_dir.display());
    println!("Master playlist: {}", master_playlist_path.display());
    
    if let Some(mut rx) = client.get_packet_receiver() {
        let mut pts = 0;
        let pts_increment = 3600; // 90kHz clock

        loop {
            match time::timeout(Duration::from_secs(5), rx.recv()).await {
                Ok(Some(data)) => {
                    // Create base packet
                    let packet = Packet::new(Bytes::from(data))
                        .with_pts(pts)
                        .with_stream_index(0);
                    pts += pts_increment;

                    // Process packet for each variant
                    for config in &mut variant_configs {
                        if config.segmenter.should_start_new_segment(Duration::from_millis(pts as u64 / 90)) {
                            let _segment_file = config.segmenter.start_segment(Duration::from_millis(pts as u64 / 90)).await?;
                            
                            // Transcode packet for this variant
                            let transcoded_packets = config.transcoder.transcode_packet(packet.clone()).await?;
                            for p in transcoded_packets {
                                config.muxer.write_packet(&p).await?;
                            }
                            
                            config.segmenter.finish_segment(Duration::from_millis((pts + pts_increment) as u64 / 90)).await?;
                            
                            // Update variant playlist
                            let playlist_path = hls_output_dir.join(format!("{}.m3u8", config.name));
                            let mut playlist_file = File::create(playlist_path).await?;
                            config.segmenter.write_playlist(&mut playlist_file).await?;
                        } else {
                            // Transcode packet for this variant
                            let transcoded_packets = config.transcoder.transcode_packet(packet.clone()).await?;
                            for p in transcoded_packets {
                                config.muxer.write_packet(&p).await?;
                            }
                        }
                    }

                    // Update master playlist periodically
                    if pts % (pts_increment * 30) == 0 { // Every 30 frames
                        variant_configs[0].segmenter.write_master_playlist(&mut master_playlist_file).await?;
                    }
                }
                Ok(None) => {
                    println!("RTSP stream ended");
                    break;
                }
                Err(_) => {
                    println!("Packet receive timeout, attempting reconnect");
                    if client.reconnect().await? {
                        client.play().await?;
                        if let Some(new_rx) = client.get_packet_receiver() {
                            rx = new_rx;
                            continue;
                        }
                    }
                    println!("Reconnection failed, ending stream");
                    break;
                }
            }
        }
    }
    
    // Clean up
    client.teardown().await?;
    
    Ok(())
}