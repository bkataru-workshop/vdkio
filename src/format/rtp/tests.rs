use super::*;

#[test]
fn test_rtp_packet_creation() {
    let payload = Bytes::from(vec![1, 2, 3, 4]);
    let packet = RTPPacket::new(
        96, // payload type
        1000, // sequence number
        90000, // timestamp
        0x12345678, // ssrc
        true, // marker
        payload.clone(),
    );

    assert_eq!(packet.version, 2);
    assert_eq!(packet.payload_type, 96);
    assert_eq!(packet.sequence_number, 1000);
    assert_eq!(packet.timestamp, 90000);
    assert_eq!(packet.ssrc, 0x12345678);
    assert_eq!(packet.marker, true);
    assert_eq!(packet.payload, payload);
}

#[test]
fn test_rtp_packet_parse() {
    let mut data = vec![
        0x80, 0xe0, 0x03, 0xe8, // V=2, P=0, X=0, CC=0, M=1, PT=96, seq=1000
        0x00, 0x01, 0x5f, 0x90, // timestamp=90000
        0x12, 0x34, 0x56, 0x78, // SSRC=0x12345678
        0x01, 0x02, 0x03, 0x04, // payload
    ];

    let packet = RTPPacket::parse(&data).unwrap();
    
    assert_eq!(packet.version, 2);
    assert_eq!(packet.padding, false);
    assert_eq!(packet.extension, false);
    assert_eq!(packet.csrc_count, 0);
    assert_eq!(packet.marker, true);
    assert_eq!(packet.payload_type, 96);
    assert_eq!(packet.sequence_number, 1000);
    assert_eq!(packet.timestamp, 90000);
    assert_eq!(packet.ssrc, 0x12345678);
    assert_eq!(&packet.payload[..], &[1, 2, 3, 4]);
}

#[test]
fn test_rtp_packet_parse_with_extension() {
    let mut data = vec![
        0x90, 0xe0, 0x03, 0xe8, // V=2, P=0, X=1, CC=0, M=1, PT=96, seq=1000
        0x00, 0x01, 0x5f, 0x90, // timestamp=90000
        0x12, 0x34, 0x56, 0x78, // SSRC=0x12345678
        0xbe, 0xde, 0x00, 0x01, // Extension header (0xbede, length=1)
        0x00, 0x00, 0x00, 0x00, // Extension data (4 bytes)
        0x01, 0x02, 0x03, 0x04, // payload
    ];

    let packet = RTPPacket::parse(&data).unwrap();
    
    assert_eq!(packet.version, 2);
    assert_eq!(packet.extension, true);
    assert!(packet.extension_data.is_some());
    let (ext_header, ext_data) = packet.extension_data.unwrap();
    assert_eq!(ext_header, 0xbede);
    assert_eq!(ext_data.len(), 4);
}

#[test]
fn test_jitter_buffer() {
    let mut jb = JitterBuffer::new(16);

    // Add packets out of order
    let packets = vec![
        (1000, vec![1]),
        (1002, vec![3]),
        (1001, vec![2]),
        (1003, vec![4]),
    ];

    for (seq, payload) in packets {
        let packet = RTPPacket::new(
            96,
            seq,
            90000,
            0x12345678,
            false,
            Bytes::from(payload),
        );
        jb.push(packet).unwrap();
    }

    // Verify packets come out in order
    for i in 0..4 {
        let packet = jb.pop().unwrap();
        assert_eq!(packet.sequence_number, 1000 + i as u16);
        assert_eq!(packet.payload[0], (i + 1) as u8);
    }

    assert!(jb.pop().is_none());
}

#[test]
fn test_jitter_buffer_overflow() {
    let mut jb = JitterBuffer::new(2);

    // Try to add more packets than buffer size
    for seq in 0..4 {
        let packet = RTPPacket::new(
            96,
            seq,
            90000,
            0x12345678,
            false,
            Bytes::from(vec![1]),
        );

        if seq < 2 {
            assert!(jb.push(packet).is_ok());
        } else {
            assert!(matches!(jb.push(packet), Err(RTPError::BufferOverflow)));
        }
    }
}

#[test]
fn test_jitter_buffer_sequence_wrap() {
    let mut jb = JitterBuffer::new(16);

    // Add packet near sequence number wrap point
    let packet1 = RTPPacket::new(
        96,
        65535, // max u16
        90000,
        0x12345678,
        false,
        Bytes::from(vec![1]),
    );
    jb.push(packet1).unwrap();

    // Add packet after wrap
    let packet2 = RTPPacket::new(
        96,
        0,
        90000,
        0x12345678,
        false,
        Bytes::from(vec![2]),
    );
    assert!(matches!(jb.push(packet2), Err(RTPError::SequenceWrapped)));
}