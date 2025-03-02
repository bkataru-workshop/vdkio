use super::transport::TransportInfo;
use crate::format::rtcp::RTCPPacket;
use crate::format::rtp::JitterBuffer;
use crate::Result;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

/// Statistics for an RTSP media stream
#[derive(Debug)]
pub struct StreamStatistics {
    /// Total number of RTP packets received
    pub packets_received: u32,
    /// Total bytes of media data received
    pub bytes_received: u64,
    /// Number of packets lost during transmission
    pub packets_lost: u32,
    /// Interarrival jitter (in timestamp units)
    pub jitter: f64,
    /// Last received sequence number
    pub last_seq: u16,
    /// Last received RTP timestamp
    pub last_timestamp: u32,
}

impl Default for StreamStatistics {
    fn default() -> Self {
        Self {
            packets_received: 0,
            bytes_received: 0,
            packets_lost: 0,
            jitter: 0.0,
            last_seq: 0,
            last_timestamp: 0,
        }
    }
}

/// Represents a media stream within an RTSP session
#[derive(Debug)]
pub struct MediaStream {
    /// Type of media (e.g., "video" or "audio")
    pub media_type: String,
    /// Stream control URL (e.g., "trackID=0")
    pub control: String,
    /// Transport configuration for the stream
    pub transport: TransportInfo,
    /// UDP socket for RTP packets (if using UDP transport)
    pub rtp_socket: Option<Arc<UdpSocket>>,
    /// UDP socket for RTCP packets (if using UDP transport)
    pub rtcp_socket: Option<Arc<UdpSocket>>,
    /// Buffer for handling out-of-order packets
    pub jitter_buffer: JitterBuffer,
    /// Stream performance statistics
    pub statistics: StreamStatistics,
    /// Channel for sending received media packets
    pub packet_sender: mpsc::Sender<Vec<u8>>,
}

impl MediaStream {
    /// Creates a new media stream with the specified configuration
    ///
    /// # Arguments
    ///
    /// * `media_type` - Type of media ("video" or "audio")
    /// * `control` - Stream control identifier
    /// * `transport` - Transport configuration
    /// * `packet_sender` - Channel for sending received packets
    ///
    /// # Example
    ///
    /// ```rust
    /// use vdkio::format::rtsp::{MediaStream, TransportInfo};
    /// use tokio::sync::mpsc;
    ///
    /// let (tx, _rx) = mpsc::channel(100);
    /// let stream = MediaStream::new(
    ///     "video",
    ///     "trackID=0",
    ///     TransportInfo::new_rtp_avp((5000, 5001)),
    ///     tx
    /// );
    /// ```
    pub fn new(
        media_type: &str,
        control: &str,
        transport: TransportInfo,
        packet_sender: mpsc::Sender<Vec<u8>>,
    ) -> Self {
        Self {
            media_type: media_type.to_string(),
            control: control.to_string(),
            transport,
            rtp_socket: None,
            rtcp_socket: None,
            jitter_buffer: JitterBuffer::new(32), // Buffer up to 32 packets
            statistics: StreamStatistics::default(),
            packet_sender,
        }
    }

    /// Configures the stream to use TCP interleaved transport
    ///
    /// # Arguments
    ///
    /// * `interleaved` - Channel numbers for RTP and RTCP data
    ///
    /// # Example
    ///
    /// ```rust
    /// # use vdkio::format::rtsp::{MediaStream, TransportInfo};
    /// # use tokio::sync::mpsc;
    /// # let (tx, _rx) = mpsc::channel(100);
    /// let stream = MediaStream::new(
    ///     "video",
    ///     "trackID=0",
    ///     TransportInfo::new_rtp_avp((0, 0)),
    ///     tx
    /// ).with_tcp_transport((0, 1)); // Use channels 0 and 1
    /// ```
    pub fn with_tcp_transport(mut self, interleaved: (u16, u16)) -> Self {
        let mut extra_params = self.transport.extra_params.clone();
        extra_params.insert(
            "interleaved".to_string(),
            Some(format!("{}-{}", interleaved.0, interleaved.1)),
        );

        self.transport = TransportInfo {
            protocol: "RTP/AVP/TCP".to_string(),
            cast_type: self.transport.cast_type,
            client_port_rtp: None, // Not used in TCP mode
            client_port_rtcp: None,
            server_port_rtp: None,
            server_port_rtcp: None,
            ssrc: self.transport.ssrc,
            mode: Some("PLAY".to_string()),
            extra_params,
        };
        self
    }

    /// Sets up UDP sockets for RTP/RTCP transport
    ///
    /// This method binds UDP sockets for receiving RTP and RTCP packets
    /// when using UDP transport mode.
    ///
    /// # Errors
    ///
    /// Returns an error if socket binding fails
    pub async fn setup_transport(&mut self) -> Result<()> {
        if self.transport.protocol != "RTP/AVP/TCP" {
            if let Some(rtp_port) = self.transport.client_port_rtp {
                // Create RTP socket
                let rtp_socket = UdpSocket::bind(format!("0.0.0.0:{}", rtp_port)).await?;
                self.rtp_socket = Some(Arc::new(rtp_socket));

                // Create RTCP socket if port is specified
                if let Some(rtcp_port) = self.transport.client_port_rtcp {
                    let rtcp_socket = UdpSocket::bind(format!("0.0.0.0:{}", rtcp_port)).await?;
                    self.rtcp_socket = Some(Arc::new(rtcp_socket));
                }
            }
        }
        Ok(())
    }

    /// Generates the transport header string for RTSP SETUP requests
    pub fn get_transport_str(&self) -> String {
        let mut transport = format!("{};unicast", self.transport.protocol);

        // For UDP mode, include port info
        if self.transport.protocol == "RTP/AVP" {
            if let (Some(rtp), Some(rtcp)) = (
                self.transport.client_port_rtp,
                self.transport.client_port_rtcp,
            ) {
                transport.push_str(&format!(";client_port={}-{}", rtp, rtcp));
            }
        }

        // For TCP mode, include interleaved channels
        if self.transport.protocol == "RTP/AVP/TCP" {
            if let Some(channels) = self.transport.extra_params.get("interleaved") {
                if let Some(channel_range) = channels {
                    transport.push_str(&format!(";interleaved={}", channel_range));
                }
            }
        }

        // Add mode if specified
        if let Some(mode) = &self.transport.mode {
            transport.push_str(&format!(";mode={}", mode));
        }

        // Add SSRC if specified
        if let Some(ssrc) = self.transport.ssrc {
            transport.push_str(&format!(";ssrc={:08x}", ssrc));
        }

        transport
    }

    /// Updates stream statistics with received packet information
    ///
    /// # Arguments
    ///
    /// * `seq` - RTP sequence number
    /// * `timestamp` - RTP timestamp
    /// * `bytes` - Packet size in bytes
    pub fn update_statistics(&mut self, seq: u16, timestamp: u32, bytes: usize) {
        let stats = &mut self.statistics;
        stats.packets_received += 1;
        stats.bytes_received += bytes as u64;

        // Handle sequence number wrapping
        if stats.last_seq != 0 {
            let expected = stats.last_seq.wrapping_add(1);
            if seq != expected {
                let gap = if seq < expected {
                    (65536 - expected as u32) + seq as u32
                } else {
                    seq as u32 - expected as u32
                };
                stats.packets_lost += gap;
            }
        }

        // Update jitter calculation (RFC 3550)
        if stats.last_timestamp != 0 {
            let d = ((timestamp as i64 - stats.last_timestamp as i64)
                - (seq as i64 - stats.last_seq as i64) * 90000) as f64; // Assuming 90kHz clock
            stats.jitter += (d.abs() - stats.jitter) / 16.0;
        }

        stats.last_seq = seq;
        stats.last_timestamp = timestamp;
    }

    /// Generates an RTCP receiver report based on current statistics
    ///
    /// # Arguments
    ///
    /// * `ssrc` - Synchronization source identifier for the report
    pub fn generate_rtcp_report(&self, ssrc: u32) -> RTCPPacket {
        let stats = &self.statistics;
        RTCPPacket::ReceiverReport {
            ssrc,
            reports: vec![crate::format::rtcp::ReceptionReport {
                ssrc,
                fraction_lost: if stats.packets_received > 0 {
                    ((stats.packets_lost * 256) / stats.packets_received) as u8
                } else {
                    0
                },
                packets_lost: stats.packets_lost,
                highest_seq: stats.last_seq as u32,
                jitter: stats.jitter as u32,
                last_sr: 0,       // Will be filled in when sending
                delay_last_sr: 0, // Will be filled in when sending
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_str_generation() {
        let transport = TransportInfo {
            protocol: "RTP/AVP".to_string(),
            cast_type: super::super::transport::CastType::Unicast,
            client_port_rtp: Some(5000),
            client_port_rtcp: Some(5001),
            server_port_rtp: None,
            server_port_rtcp: None,
            ssrc: Some(0x12345678),
            mode: None,
            extra_params: Default::default(),
        };

        let stream = MediaStream::new("video", "trackID=1", transport, mpsc::channel(1).0);

        let transport_str = stream.get_transport_str();
        assert!(transport_str.contains("RTP/AVP"));
        assert!(transport_str.contains("unicast"));
        assert!(transport_str.contains("client_port=5000-5001"));
        assert!(transport_str.contains("ssrc=12345678"));
    }

    #[test]
    fn test_tcp_transport_str_generation() {
        let mut stream = MediaStream::new(
            "video",
            "trackID=1",
            TransportInfo::new_rtp_avp((0, 0)), // Ports not used in TCP
            mpsc::channel(1).0,
        );

        stream = stream.with_tcp_transport((0, 1));
        let transport_str = stream.get_transport_str();
        assert!(transport_str.contains("RTP/AVP/TCP"));
        assert!(transport_str.contains("unicast"));
        assert!(transport_str.contains("interleaved=0-1"));
        assert!(transport_str.contains("mode=PLAY"));
    }

    #[test]
    fn test_statistics_update() {
        let transport = TransportInfo {
            protocol: "RTP/AVP".to_string(),
            cast_type: super::super::transport::CastType::Unicast,
            client_port_rtp: Some(5000),
            client_port_rtcp: Some(5001),
            server_port_rtp: None,
            server_port_rtcp: None,
            ssrc: None,
            mode: None,
            extra_params: Default::default(),
        };

        let mut stream = MediaStream::new("video", "trackID=1", transport, mpsc::channel(1).0);

        // Test normal packet sequence
        stream.update_statistics(1000, 90000, 1000);
        stream.update_statistics(1001, 90090, 1000);

        assert_eq!(stream.statistics.packets_received, 2);
        assert_eq!(stream.statistics.bytes_received, 2000);
        assert_eq!(stream.statistics.packets_lost, 0);

        // Test packet loss
        stream.update_statistics(1003, 90270, 1000);
        assert_eq!(stream.statistics.packets_lost, 1);
    }

    #[test]
    fn test_generate_rtcp_report() {
        let transport = TransportInfo::new_rtp_avp((5000, 5001));
        let mut stream = MediaStream::new("video", "trackID=1", transport, mpsc::channel(1).0);

        stream.update_statistics(1000, 90000, 1000);
        stream.update_statistics(1002, 90180, 1000); // One packet lost

        let ssrc = 0x12345678;
        let rtcp_packet = stream.generate_rtcp_report(ssrc);

        if let RTCPPacket::ReceiverReport {
            ssrc: pkt_ssrc,
            reports,
        } = rtcp_packet
        {
            assert_eq!(pkt_ssrc, ssrc);
            assert_eq!(reports.len(), 1);
            assert_eq!(reports[0].ssrc, ssrc);
            assert_eq!(reports[0].fraction_lost, 128); // 50% loss (1/2 * 256)
            assert_eq!(reports[0].packets_lost, 1);
            assert_eq!(reports[0].highest_seq, 1002);
        } else {
            panic!("Expected ReceiverReport");
        }
    }
}
