use super::types::time_to_pts;
use crate::error::Result;
use bytes::{BufMut, BytesMut};
use std::time::Duration;

/// Packetized Elementary Stream (PES) header structure
///
/// Contains fields defined in the MPEG-TS specification for PES headers,
/// including timing information and various control flags.
#[derive(Debug, Clone)]
pub struct PESHeader {
    /// Start code prefix, always 0x000001 for PES packets
    pub start_code_prefix: u32,
    /// Stream identifier indicating content type (video/audio/etc.)
    pub stream_id: u8,
    /// Length of the PES packet (header + payload)
    pub packet_length: u16,
    /// Control field for scrambling mode
    pub scrambling_control: u8,
    /// Priority flag for the packet
    pub priority: bool,
    /// Data alignment indicator
    pub data_alignment: bool,
    /// Copyright indicator
    pub copyright: bool,
    /// Original/copy indicator
    pub original: bool,
    /// Flags indicating presence of PTS/DTS fields
    pub pts_dts_flags: u8,
    /// ESCR (Extended System Clock Reference) flag
    pub escr_flag: bool,
    /// Elementary Stream rate flag
    pub es_rate_flag: bool,
    /// DSM trick mode flag
    pub dsm_trick_mode_flag: bool,
    /// Additional copy info flag
    pub additional_copy_info_flag: bool,
    /// CRC flag
    pub crc_flag: bool,
    /// Extension flag
    pub extension_flag: bool,
    /// Length of the header data following this field
    pub header_data_length: u8,
    /// Presentation Time Stamp (33 bits)
    pub pts: Option<u64>,
    /// Decoding Time Stamp (33 bits)
    pub dts: Option<u64>,
}

impl Default for PESHeader {
    fn default() -> Self {
        Self {
            start_code_prefix: 0x000001,
            stream_id: 0,
            packet_length: 0,
            scrambling_control: 0,
            priority: false,
            data_alignment: false,
            copyright: false,
            original: false,
            pts_dts_flags: 0,
            escr_flag: false,
            es_rate_flag: false,
            dsm_trick_mode_flag: false,
            additional_copy_info_flag: false,
            crc_flag: false,
            extension_flag: false,
            header_data_length: 0,
            pts: None,
            dts: None,
        }
    }
}

impl PESHeader {
    /// Creates a new PES header with a specific stream ID.
    ///
    /// # Arguments
    /// * `stream_id` - The stream_id for this PES header.
    pub fn new(stream_id: u8) -> Self {
        Self {
            stream_id,
            ..Default::default()
        }
    }

    /// Sets the Presentation Time Stamp (PTS) for the PES header.
    ///
    /// # Arguments
    /// * `pts` - The PTS Duration to set.
    ///
    /// # Returns
    /// The modified PESHeader instance.
    pub fn with_pts(mut self, pts: Duration) -> Self {
        self.pts = Some(time_to_pts(pts));
        self.pts_dts_flags |= 0x80;
        self
    }

    /// Sets the Decoding Time Stamp (DTS) for the PES header.
    ///
    /// # Arguments
    /// * `dts` - The DTS Duration to set.
    ///
    /// # Returns
    /// The modified PESHeader instance.
    pub fn with_dts(mut self, dts: Duration) -> Self {
        self.dts = Some(time_to_pts(dts));
        self.pts_dts_flags |= 0x40;
        self
    }

    /// Writes the PES header to a BytesMut buffer.
    ///
    /// This method encodes all header fields into the buffer according to MPEG-TS PES syntax.
    ///
    /// # Arguments
    /// * `buf` - The BytesMut buffer to write the header to.
    ///
    /// # Returns
    /// `Ok(())` if writing is successful, `Err` otherwise.
    pub fn write_to(&self, buf: &mut BytesMut) -> Result<()> {
        // Start code prefix (3 bytes) - manually writing 24 bits
        buf.put_u8((self.start_code_prefix >> 16) as u8);
        buf.put_u8((self.start_code_prefix >> 8) as u8);
        buf.put_u8(self.start_code_prefix as u8);

        // Stream ID (1 byte)
        buf.put_u8(self.stream_id);

        // PES packet length (2 bytes)
        buf.put_u16(self.packet_length);

        // Flags (1 byte)
        let mut flags = 0u8;
        flags |= self.scrambling_control << 6;
        if self.priority {
            flags |= 0x20;
        }
        if self.data_alignment {
            flags |= 0x10;
        }
        if self.copyright {
            flags |= 0x08;
        }
        if self.original {
            flags |= 0x04;
        }
        flags |= self.pts_dts_flags;
        buf.put_u8(flags);

        // Additional flags (1 byte)
        let mut flags2 = 0u8;
        if self.escr_flag {
            flags2 |= 0x20;
        }
        if self.es_rate_flag {
            flags2 |= 0x10;
        }
        if self.dsm_trick_mode_flag {
            flags2 |= 0x08;
        }
        if self.additional_copy_info_flag {
            flags2 |= 0x04;
        }
        if self.crc_flag {
            flags2 |= 0x02;
        }
        if self.extension_flag {
            flags2 |= 0x01;
        }
        buf.put_u8(flags2);

        // Header data length (1 byte)
        buf.put_u8(self.header_data_length);

        // Write PTS if present
        if let Some(pts) = self.pts {
            let marker = if self.dts.is_some() { 0x30 } else { 0x20 };
            write_timestamp(buf, marker, pts)?;
        }

        // Write DTS if present
        if let Some(dts) = self.dts {
            write_timestamp(buf, 0x10, dts)?;
        }

        Ok(())
    }
}

/// Represents a complete Packetized Elementary Stream (PES) packet.
///
/// A PES packet consists of a header and payload data. It's used to carry
/// compressed video, audio, or other data in an MPEG transport stream.
#[derive(Debug)]
pub struct PESPacket {
    /// PES header containing metadata and flags
    pub header: PESHeader,
    /// Actual payload data of the PES packet
    pub payload: Vec<u8>,
}

impl PESPacket {
    /// Creates a new PES packet with the specified stream ID and payload data.
    ///
    /// # Arguments
    /// * `stream_id` - The stream identifier (e.g., video or audio)
    /// * `payload` - The actual packet payload data
    ///
    /// # Returns
    /// A new PESPacket instance
    pub fn new(stream_id: u8, payload: Vec<u8>) -> Self {
        let header = PESHeader::new(stream_id);
        Self { header, payload }
    }

    /// Sets the Presentation Time Stamp (PTS) for the PES packet.
    ///
    /// # Arguments
    /// * `pts` - Presentation Time Stamp for the PES packet
    ///
    /// # Returns
    /// The modified PESPacket instance
    pub fn with_pts(mut self, pts: Duration) -> Self {
        self.header = self.header.with_pts(pts);
        self
    }

    /// Sets the Decoding Time Stamp (DTS) for the PES packet.
    ///
    /// # Arguments
    /// * `dts` - Decoding Time Stamp for the PES packet
    ///
    /// # Returns
    /// The modified PESPacket instance
    pub fn with_dts(mut self, dts: Duration) -> Self {
        self.header = self.header.with_dts(dts);
        self
    }

    /// Writes the complete PES packet to a BytesMut buffer.
    ///
    /// # Arguments
    /// * `buf` - The BytesMut buffer to write the PES packet to
    ///
    /// # Returns
    /// `Ok(())` if writing is successful, `Err` otherwise
    pub fn write_to(&self, buf: &mut BytesMut) -> Result<()> {
        self.header.write_to(buf)?;
        buf.extend_from_slice(&self.payload);
        Ok(())
    }

    /// Returns the total length of the PES packet in bytes.
    ///
    /// This includes the fixed header size, optional PTS/DTS fields,
    /// and the payload length.
    ///
    /// # Returns
    /// Length in bytes of the complete PES packet
    pub fn len(&self) -> usize {
        9 + // Fixed PES header size
        (if self.header.pts.is_some() { 5 } else { 0 }) + // PTS size
        (if self.header.dts.is_some() { 5 } else { 0 }) + // DTS size
        self.payload.len()
    }
}

/// Helper function to write PTS/DTS timestamps to a buffer.
///
/// # Arguments
/// * `buf` - The BytesMut buffer to write to
/// * `marker` - Marker bits for the timestamp
/// * `ts` - The timestamp value to write
///
/// # Returns
/// `Ok(())` if writing is successful, `Err` otherwise
fn write_timestamp(buf: &mut BytesMut, marker: u8, ts: u64) -> Result<()> {
    let pts = ts & 0x1FFFFFFFF; // 33 bits

    // First byte: marker bits and 3 MSB of timestamp
    buf.put_u8(marker | ((pts >> 29) & 0x0E) as u8 | 0x01);

    // Middle 16 bits and marker
    buf.put_u16((((pts >> 14) & 0xFFFE) | 0x01) as u16);

    // Final 15 bits and marker
    buf.put_u16((((pts << 1) & 0xFFFE) | 0x01) as u16);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pes_packet_creation() {
        let payload = vec![0; 10];
        let packet = PESPacket::new(0xe0, payload.clone())
            .with_pts(Duration::from_secs(1))
            .with_dts(Duration::from_secs(1));

        assert_eq!(packet.header.stream_id, 0xe0);
        assert_eq!(packet.payload, payload);
        assert!(packet.header.pts.is_some());
        assert!(packet.header.dts.is_some());
    }

    #[test]
    fn test_pes_packet_writing() {
        let mut buf = BytesMut::new();
        let payload = vec![0; 10];
        let packet = PESPacket::new(0xe0, payload).with_pts(Duration::from_secs(1));

        packet.write_to(&mut buf).unwrap();

        // Verify start code prefix
        assert_eq!(&buf[0..3], &[0x00, 0x00, 0x01]);

        // Verify stream ID
        assert_eq!(buf[3], 0xe0);
    }
}
