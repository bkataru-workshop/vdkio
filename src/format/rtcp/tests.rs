use super::*;

#[test]
fn test_rtcp_sender_report_parse() {
    let data = vec![
        0x80, 0xc8, 0x00, 0x06, // V=2, P=0, Count=0, Type=SR(200), Len=6
        0x12, 0x34, 0x56, 0x78, // SSRC
        0xdf, 0xa0, 0x00, 0x00, // NTP timestamp (MSW)
        0x00, 0x00, 0x00, 0x00, // NTP timestamp (LSW)
        0x00, 0x01, 0x86, 0xa0, // RTP timestamp (100000)
        0x00, 0x00, 0x00, 0x0a, // Packet count (10)
        0x00, 0x00, 0x0b, 0xb8, // Octet count (3000)
    ];

    let packet = RTCPPacket::parse(&data).unwrap();
    match packet {
        RTCPPacket::SenderReport { 
            ssrc,
            ntp_timestamp,
            rtp_timestamp,
            packet_count,
            octet_count,
            reports
        } => {
            assert_eq!(ssrc, 0x12345678);
            assert_eq!(rtp_timestamp, 100000);
            assert_eq!(packet_count, 10);
            assert_eq!(octet_count, 3000);
            assert!(reports.is_empty());
        },
        _ => panic!("Expected SenderReport"),
    }
}

#[test]
fn test_rtcp_receiver_report_parse() {
    let data = vec![
        0x81, 0xc9, 0x00, 0x07, // V=2, P=0, Count=1, Type=RR(201), Len=7
        0x12, 0x34, 0x56, 0x78, // SSRC of packet sender
        0x11, 0x11, 0x11, 0x11, // SSRC_1 (source 1)
        0x20, 0x00, 0x00, 0x01, // fraction lost + cumulative lost
        0x00, 0x00, 0x03, 0xe8, // extended highest seq number
        0x00, 0x00, 0x00, 0x64, // interarrival jitter
        0x00, 0x00, 0x00, 0x00, // LSR
        0x00, 0x00, 0x00, 0x00, // DLSR
    ];

    let packet = RTCPPacket::parse(&data).unwrap();
    match packet {
        RTCPPacket::ReceiverReport { ssrc, reports } => {
            assert_eq!(ssrc, 0x12345678);
            assert_eq!(reports.len(), 1);
            
            let report = &reports[0];
            assert_eq!(report.ssrc, 0x11111111);
            assert_eq!(report.fraction_lost, 0x20);
            assert_eq!(report.packets_lost, 1);
            assert_eq!(report.highest_seq, 1000);
            assert_eq!(report.jitter, 100);
        },
        _ => panic!("Expected ReceiverReport"),
    }
}

#[test]
fn test_ntp_timestamp() {
    let ts = get_ntp_timestamp();
    
    // Basic validation of NTP timestamp
    assert!(ts > 0);
    
    // NTP timestamp should be after Jan 1, 2020 (in NTP format)
    let jan_2020_ntp = (3786825600u64) << 32; // Seconds between 1900 and 2020
    assert!(ts > jan_2020_ntp);
}

#[test]
fn test_invalid_rtcp_packet() {
    // Test too short packet
    let data = vec![0x80, 0xc8, 0x00];
    assert!(matches!(RTCPPacket::parse(&data), Err(RTCPError::InvalidPacket)));

    // Test invalid version
    let data = vec![
        0x40, 0xc8, 0x00, 0x06, // V=1 (invalid), P=0, Count=0, Type=SR(200)
        0x00, 0x00, 0x00, 0x00,
    ];
    assert!(matches!(RTCPPacket::parse(&data), Err(RTCPError::InvalidPacket)));

    // Test invalid packet type
    let data = vec![
        0x80, 0xff, 0x00, 0x06, // V=2, P=0, Count=0, Type=255 (invalid)
        0x00, 0x00, 0x00, 0x00,
    ];
    assert!(matches!(RTCPPacket::parse(&data), Err(RTCPError::UnsupportedType)));
}

#[test]
fn test_reception_report_parse() {
    let data = vec![
        0x11, 0x11, 0x11, 0x11, // SSRC
        0x20, 0x00, 0x00, 0x01, // fraction lost + cumulative lost
        0x00, 0x00, 0x03, 0xe8, // extended highest seq number
        0x00, 0x00, 0x00, 0x64, // interarrival jitter
        0x12, 0x34, 0x56, 0x78, // LSR
        0x00, 0x00, 0x00, 0x0a, // DLSR
    ];

    let report = parse_reception_report(&data).unwrap();
    assert_eq!(report.ssrc, 0x11111111);
    assert_eq!(report.fraction_lost, 0x20);
    assert_eq!(report.packets_lost, 1);
    assert_eq!(report.highest_seq, 1000);
    assert_eq!(report.jitter, 100);
    assert_eq!(report.last_sr, 0x12345678);
    assert_eq!(report.delay_last_sr, 10);
}

#[test]
fn test_reception_report_invalid() {
    let data = vec![0x00, 0x00, 0x00]; // Too short
    assert!(matches!(parse_reception_report(&data), Err(RTCPError::InvalidPacket)));
}