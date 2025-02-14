use bytes::Bytes;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RTCPError {
    #[error("Invalid RTCP packet")]
    InvalidPacket,
    #[error("Unsupported packet type")]
    UnsupportedType,
}

pub type Result<T> = std::result::Result<T, RTCPError>;

#[derive(Debug, Clone)]
pub struct ReceptionReport {
    /// SSRC of the source this report is for
    pub ssrc: u32,
    /// Fraction of RTP data packets lost since the previous SR/RR
    pub fraction_lost: u8,
    /// Cumulative number of packets lost
    pub packets_lost: u32,
    /// Extended highest sequence number received
    pub highest_seq: u32,
    /// Interarrival jitter
    pub jitter: u32,
    /// Last SR timestamp (LSR)
    pub last_sr: u32,
    /// Delay since last SR (DLSR)
    pub delay_last_sr: u32,
}

#[derive(Debug)]
pub enum RTCPPacket {
    SenderReport {
        ssrc: u32,
        ntp_timestamp: u64,
        rtp_timestamp: u32,
        packet_count: u32,
        octet_count: u32,
        reports: Vec<ReceptionReport>,
    },
    ReceiverReport {
        ssrc: u32,
        reports: Vec<ReceptionReport>,
    },
    SourceDescription {
        chunks: Vec<(u32, Vec<(u8, String)>)>,
    },
    Goodbye {
        sources: Vec<u32>,
        reason: Option<String>,
    },
    ApplicationDefined {
        ssrc: u32,
        name: [u8; 4],
        data: Bytes,
    },
}

impl RTCPPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(RTCPError::InvalidPacket);
        }

        let first_byte = data[0];
        let packet_type = data[1];

        // Check version
        let version = (first_byte >> 6) & 0x03;
        if version != 2 {
            return Err(RTCPError::InvalidPacket);
        }

        let padding = (first_byte & 0x20) != 0;
        let count = first_byte & 0x1f;
        
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;
        if data.len() < (length + 1) * 4 {
            return Err(RTCPError::InvalidPacket);
        }

        let mut offset = 4;
        let payload_end = if padding {
            let padding_len = data[data.len() - 1] as usize;
            data.len() - padding_len
        } else {
            data.len()
        };

        match packet_type {
            200 => { // Sender Report
                if payload_end - offset < 20 {
                    return Err(RTCPError::InvalidPacket);
                }

                let ssrc = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                let ntp_msw = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                let ntp_lsw = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                let ntp_timestamp = ((ntp_msw as u64) << 32) | (ntp_lsw as u64);

                let rtp_timestamp = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                let packet_count = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                let octet_count = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                let mut reports = Vec::new();
                for _ in 0..count {
                    if payload_end - offset < 24 {
                        return Err(RTCPError::InvalidPacket);
                    }

                    let report = parse_reception_report(&data[offset..offset + 24])?;
                    offset += 24;
                    reports.push(report);
                }

                Ok(RTCPPacket::SenderReport {
                    ssrc,
                    ntp_timestamp,
                    rtp_timestamp,
                    packet_count,
                    octet_count,
                    reports,
                })
            }
            201 => { // Receiver Report
                if payload_end - offset < 4 {
                    return Err(RTCPError::InvalidPacket);
                }

                let ssrc = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                let mut reports = Vec::new();
                for _ in 0..count {
                    if payload_end - offset < 24 {
                        return Err(RTCPError::InvalidPacket);
                    }

                    let report = parse_reception_report(&data[offset..offset + 24])?;
                    offset += 24;
                    reports.push(report);
                }

                Ok(RTCPPacket::ReceiverReport { ssrc, reports })
            }
            // Add other packet types as needed...
            _ => Err(RTCPError::UnsupportedType),
        }
    }
}

fn parse_reception_report(data: &[u8]) -> Result<ReceptionReport> {
    if data.len() < 24 {
        return Err(RTCPError::InvalidPacket);
    }

    let ssrc = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let fraction_lost = data[4];
    let packets_lost = u32::from_be_bytes([0, data[5], data[6], data[7]]) & 0x00FF_FFFF;
    let highest_seq = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let jitter = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
    let last_sr = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let delay_last_sr = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);

    Ok(ReceptionReport {
        ssrc,
        fraction_lost,
        packets_lost,
        highest_seq,
        jitter,
        last_sr,
        delay_last_sr,
    })
}

/// Get current NTP timestamp (64-bit fixed point)
pub fn get_ntp_timestamp() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    
    // Convert to NTP format (seconds since 1900)
    let ntp_seconds = now.as_secs() + 2_208_988_800; // Seconds between 1900 and 1970
    let ntp_fraction = ((now.subsec_nanos() as u64) << 32) / 1_000_000_000;
    
    (ntp_seconds << 32) | ntp_fraction
}