use vdkio::prelude::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn test_h264_and_rtsp_integration() -> Result<()> {
    // Create a basic H264 parser
    let mut h264_parser = H264Parser::new();

    // Test basic NAL unit parsing
    // Test data for a valid H.264 SPS NAL unit (1920x1080 resolution)
    let test_data = vec![
        0x00, 0x00, 0x00, 0x01, 0x67,  // Start code and SPS NAL type
        0x42, 0x80, 0x1f,              // Profile, constraints, and level
        0x8d, 0x8d, 0x40,              // SPS ID and other params
        0x50, 0x1e, 0x84, 0x00,        // Resolution related
        0x00, 0x4f, 0x08,              // More params
        0x00, 0x00, 0x01, 0x68,        // Start code for next NAL
        0xe8, 0x43, 0x8f, 0x13, 0x21,  // More params
        0x30                           // End
    ];

    let nalu = h264_parser.parse_nalu(&test_data[4..])?;
    assert_eq!(nalu.nal_type, 7); // SPS type
    assert!(nalu.is_keyframe());

    // Test RTSP URL parsing
    let rtsp_result = RTSPClient::new("rtsp://example.com:8554/test");
    assert!(rtsp_result.is_ok());

    // Test SDP parsing
    let sdp_str = "\
v=0
o=- 123 456 IN IP4 127.0.0.1
s=Test Stream
c=IN IP4 127.0.0.1
t=0 0
m=video 5000 RTP/AVP 96
a=rtpmap:96 H264/90000
a=fmtp:96 profile-level-id=42e01f
m=audio 5002 RTP/AVP 97
a=rtpmap:97 MPEG4-GENERIC/44100/2
";

    let session = SessionDescription::parse(sdp_str)?;
    assert_eq!(session.media.len(), 2);

    let video = session.get_media("video").unwrap();
    assert_eq!(video.port, 5000);
    assert_eq!(video.protocol, "RTP/AVP");

    Ok(())
}

#[tokio::test]
async fn test_aac_parser() -> Result<()> {
    let mut parser = AACParser::new();
    
    // ADTS frame header for AAC-LC, stereo, 44.1kHz
    let test_data = vec![
        0xFF, 0xF1, // Sync word + ID + Layer + Protection
        0x50, // Profile + Sampling + Private
        0x80, // Channel + Original + Home
        0x43, 0x80, // Frame length
        0x00, // Buffer + Raw blocks
        0x1C, 0x01, // Some sample AAC data
    ];

    let frame = parser.parse_frame(&test_data)?;
    assert_eq!(frame.config.channel_configuration, 2); // Stereo
    assert_eq!(frame.config.sample_rate_index, 4); // 44.1kHz

    Ok(())
}
