use bytes::Bytes;
use std::collections::BTreeMap;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RTPError {
    #[error("Invalid RTP packet")]
    InvalidPacket,
    #[error("Buffer overflow")]
    BufferOverflow,
    #[error("Sequence number wrapped")]
    SequenceWrapped,
}

pub type Result<T> = std::result::Result<T, RTPError>;

#[derive(Debug, Clone)]
pub struct RTPPacket {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub csrc_count: u8,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub csrc: Vec<u32>,
    pub extension_data: Option<(u16, Bytes)>,
    pub payload: Bytes,
}

impl RTPPacket {
    pub fn new(
        payload_type: u8,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
        marker: bool,
        payload: Bytes,
    ) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            csrc: Vec::new(),
            extension_data: None,
            payload,
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(RTPError::InvalidPacket);
        }

        let first_byte = data[0];
        let second_byte = data[1];

        let version = (first_byte >> 6) & 0x03;
        if version != 2 {
            return Err(RTPError::InvalidPacket);
        }

        let padding = (first_byte & 0x20) != 0;
        let extension = (first_byte & 0x10) != 0;
        let csrc_count = first_byte & 0x0f;

        let marker = (second_byte & 0x80) != 0;
        let payload_type = second_byte & 0x7f;

        let sequence_number = u16::from_be_bytes([data[2], data[3]]);
        let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let mut offset = 12;

        let mut csrc = Vec::with_capacity(csrc_count as usize);
        for _ in 0..csrc_count {
            if offset + 4 > data.len() {
                return Err(RTPError::InvalidPacket);
            }
            csrc.push(u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
        }

        let extension_data = if extension {
            if offset + 4 > data.len() {
                return Err(RTPError::InvalidPacket);
            }
            let ext_header = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let ext_length = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize * 4;
            offset += 4;

            if offset + ext_length > data.len() {
                return Err(RTPError::InvalidPacket);
            }
            let ext_data = Bytes::copy_from_slice(&data[offset..offset + ext_length]);
            offset += ext_length;
            Some((ext_header, ext_data))
        } else {
            None
        };

        let payload = if padding {
            let padding_len = data[data.len() - 1] as usize;
            if padding_len == 0 || offset + padding_len > data.len() {
                return Err(RTPError::InvalidPacket);
            }
            Bytes::copy_from_slice(&data[offset..data.len() - padding_len])
        } else {
            Bytes::copy_from_slice(&data[offset..])
        };

        Ok(Self {
            version: 2,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            csrc,
            extension_data,
            payload,
        })
    }
}

pub struct JitterBuffer {
    packets: BTreeMap<u16, RTPPacket>,
    min_seq: u16,
    max_seq: u16,
    buffer_size: usize,
}

impl fmt::Debug for JitterBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JitterBuffer")
            .field("min_seq", &self.min_seq)
            .field("max_seq", &self.max_seq)
            .field("buffer_size", &self.buffer_size)
            .field("packet_count", &self.packets.len())
            .finish()
    }
}

impl JitterBuffer {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            packets: BTreeMap::new(),
            min_seq: 0,
            max_seq: 0,
            buffer_size,
        }
    }

    pub fn push(&mut self, packet: RTPPacket) -> Result<()> {
        let seq = packet.sequence_number;

        if self.packets.is_empty() {
            self.min_seq = seq;
            self.max_seq = seq;
            self.packets.insert(seq, packet);
            return Ok(());
        }

        // Handle sequence number wrapping
        if (seq < 0x4000 && self.max_seq > 0xC000) || (seq > 0xC000 && self.min_seq < 0x4000) {
            return Err(RTPError::SequenceWrapped);
        }

        // Update sequence bounds
        if seq < self.min_seq {
            self.min_seq = seq;
        }
        if seq > self.max_seq {
            self.max_seq = seq;
        }

        // Check buffer size
        if self.packets.len() >= self.buffer_size {
            return Err(RTPError::BufferOverflow);
        }

        self.packets.insert(seq, packet);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<RTPPacket> {
        if let Some((&seq, _)) = self.packets.first_key_value() {
            if seq == self.min_seq {
                let packet = self.packets.remove(&seq)?;
                self.min_seq = self.min_seq.wrapping_add(1);
                return Some(packet);
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtp_packet_creation() {
        let payload = Bytes::from(vec![1, 2, 3, 4]);
        let packet = RTPPacket::new(96, 1000, 90000, 0x12345678, true, payload.clone());

        assert_eq!(packet.version, 2);
        assert_eq!(packet.payload_type, 96);
        assert_eq!(packet.sequence_number, 1000);
        assert_eq!(packet.timestamp, 90000);
        assert_eq!(packet.ssrc, 0x12345678);
        assert_eq!(packet.marker, true);
        assert_eq!(packet.payload, payload);
    }

    #[test]
    fn test_jitter_buffer_operations() {
        let mut jb = JitterBuffer::new(16);

        // Add packets out of order
        let packets = vec![
            (1000, vec![1]),
            (1002, vec![3]),
            (1001, vec![2]),
            (1003, vec![4]),
        ];

        for (seq, payload) in packets {
            let packet = RTPPacket::new(96, seq, 90000, 0x12345678, false, Bytes::from(payload));
            jb.push(packet).unwrap();
        }

        // Verify packets come out in order
        for i in 0..4 {
            let packet = jb.pop().unwrap();
            assert_eq!(packet.sequence_number, 1000 + i as u16);
            assert_eq!(packet.payload[0], (i + 1) as u8);
        }

        assert!(jb.is_empty());
    }
}
