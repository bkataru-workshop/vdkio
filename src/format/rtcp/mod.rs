//! # RTP Control Protocol (RTCP) Implementation
//!
//! This module provides support for RTCP (RTP Control Protocol) packet handling.
//! RTCP works alongside RTP to provide feedback on the quality of data distribution
//! and participant session information.
//!
//! ## Features
//!
//! - Sender and Receiver Report parsing/generation
//! - Reception statistics tracking
//! - Session participant information handling
//! - NTP timestamp utilities
//!
//! ## Example
//!
//! ```rust
//! use vdkio::format::rtcp::{RTCPPacket, ReceptionReport};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a receiver report
//! let report = RTCPPacket::ReceiverReport {
//!     ssrc: 0x12345678,
//!     reports: vec![
//!         ReceptionReport {
//!             ssrc: 0x87654321,
//!             fraction_lost: 0,
//!             packets_lost: 0,
//!             highest_seq: 1000,
//!             jitter: 0,
//!             last_sr: 0,
//!             delay_last_sr: 0,
//!         }
//!     ],
//! };
//! # Ok(())
//! # }
//! ```

use bytes::Bytes;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Errors that can occur during RTCP packet operations
#[derive(Debug, Error)]
pub enum RTCPError {
    /// The packet data is malformed or incomplete
    #[error("Invalid RTCP packet")]
    InvalidPacket,
    
    /// The packet type is not supported by this implementation
    #[error("Unsupported packet type")]
    UnsupportedType,
}

/// Specialized Result type for RTCP operations
pub type Result<T> = std::result::Result<T, RTCPError>;

/// Reception statistics for an RTP source
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

/// Different types of RTCP packets
#[derive(Debug)]
pub enum RTCPPacket {
    /// Sender Report (SR) packet, containing transmission and reception statistics
    SenderReport {
        /// Synchronization source identifier
        ssrc: u32,
        /// NTP timestamp in 64-bit fixed point format
        ntp_timestamp: u64,
        /// RTP timestamp corresponding to NTP timestamp
        rtp_timestamp: u32,
        /// Total number of packets sent
        packet_count: u32,
        /// Total number of payload octets sent
        octet_count: u32,
        /// Reception reports for other sources
        reports: Vec<ReceptionReport>,
    },

    /// Receiver Report (RR) packet, containing reception statistics
    ReceiverReport {
        /// Synchronization source identifier
        ssrc: u32,
        /// Reception reports for other sources
        reports: Vec<ReceptionReport>,
    },

    /// Source Description (SDES) packet
    SourceDescription {
        /// List of (SSRC, item list) pairs. Each item is (type, value)
        chunks: Vec<(u32, Vec<(u8, String)>)>,
    },

    /// Goodbye (BYE) packet
    Goodbye {
        /// List of sources leaving the session
        sources: Vec<u32>,
        /// Optional reason for leaving
        reason: Option<String>,
    },

    /// Application-Defined (APP) packet
    ApplicationDefined {
        /// Source identifier
        ssrc: u32,
        /// Four-character name
        name: [u8; 4],
        /// Application-specific data
        data: Bytes,
    },
}

impl RTCPPacket {
    /// Parse an RTCP packet from raw bytes
    ///
    /// # Arguments
    ///
    /// * `data` - Raw packet data
    ///
    /// # Returns
    ///
    /// The parsed RTCP packet
    ///
    /// # Errors
    ///
    /// Returns `RTCPError` if:
    /// - The packet is shorter than 4 bytes
    /// - The version is not 2
    /// - The packet length is invalid
    /// - The packet type is unsupported
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
            200 => {
                // Sender Report
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
            201 => {
                // Receiver Report
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

/// Parse a reception report block from raw data
///
/// # Arguments
///
/// * `data` - Raw report block data (must be 24 bytes)
///
/// # Returns
///
/// The parsed reception report
///
/// # Errors
///
/// Returns `RTCPError::InvalidPacket` if the data is not 24 bytes long
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
///
/// Returns the current time as an NTP timestamp in 64-bit fixed point format
/// (seconds since January 1, 1900).
///
/// # Example
///
/// ```rust
/// use vdkio::format::rtcp::get_ntp_timestamp;
///
/// let ntp_now = get_ntp_timestamp();
/// println!("Current NTP timestamp: {}", ntp_now);
/// ```
pub fn get_ntp_timestamp() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    // Convert to NTP format (seconds since 1900)
    let ntp_seconds = now.as_secs() + 2_208_988_800; // Seconds between 1900 and 1970
    let ntp_fraction = ((now.subsec_nanos() as u64) << 32) / 1_000_000_000;

    (ntp_seconds << 32) | ntp_fraction
}
