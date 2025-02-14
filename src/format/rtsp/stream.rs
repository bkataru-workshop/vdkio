use crate::format::rtp::JitterBuffer;
use crate::format::rtcp::RTCPPacket;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use super::transport::TransportInfo;
use crate::Result;
use std::sync::Arc;

#[derive(Debug)]
pub struct StreamStatistics {
    pub packets_received: u32,
    pub bytes_received: u64,
    pub packets_lost: u32,
    pub jitter: f64,
    pub last_seq: u16,
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

#[derive(Debug)]
pub struct MediaStream {
    pub media_type: String,
    pub control: String,
    pub transport: TransportInfo,
    pub rtp_socket: Option<Arc<UdpSocket>>,
    pub rtcp_socket: Option<Arc<UdpSocket>>,
    pub jitter_buffer: JitterBuffer,
    pub statistics: StreamStatistics,
    pub packet_sender: mpsc::Sender<Vec<u8>>,
}

impl MediaStream {
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

    pub async fn setup_transport(&mut self) -> Result<()> {
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
        Ok(())
    }

    pub fn get_transport_str(&self) -> String {
        let mut transport = format!("{};unicast", self.transport.protocol);
        
        if let (Some(rtp), Some(rtcp)) = (self.transport.client_port_rtp, self.transport.client_port_rtcp) {
            transport.push_str(&format!(";client_port={}-{}", rtp, rtcp));
        }

        if let Some(ssrc) = self.transport.ssrc {
            transport.push_str(&format!(";ssrc={:08x}", ssrc));
        }

        transport
    }

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
            let d = ((timestamp as i64 - stats.last_timestamp as i64) -
                    (seq as i64 - stats.last_seq as i64) * 90000) as f64; // Assuming 90kHz clock
            stats.jitter += (d.abs() - stats.jitter) / 16.0;
        }

        stats.last_seq = seq;
        stats.last_timestamp = timestamp;
    }

    pub fn generate_rtcp_report(&self, ssrc: u32) -> RTCPPacket {
        let stats = &self.statistics;
        RTCPPacket::ReceiverReport {
            ssrc,
            reports: vec![crate::format::rtcp::ReceptionReport {
                ssrc: ssrc,
                fraction_lost: if stats.packets_received > 0 {
                    ((stats.packets_lost * 256) / stats.packets_received) as u8
                } else {
                    0
                },
                packets_lost: stats.packets_lost,
                highest_seq: stats.last_seq as u32,
                jitter: stats.jitter as u32,
                last_sr: 0, // Will be filled in when sending
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

        let stream = MediaStream::new(
            "video",
            "trackID=1",
            transport,
            mpsc::channel(1).0,
        );

        let transport_str = stream.get_transport_str();
        assert!(transport_str.contains("RTP/AVP"));
        assert!(transport_str.contains("unicast"));
        assert!(transport_str.contains("client_port=5000-5001"));
        assert!(transport_str.contains("ssrc=12345678"));
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

        let mut stream = MediaStream::new(
            "video",
            "trackID=1",
            transport,
            mpsc::channel(1).0,
        );

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
        let mut stream = MediaStream::new(
            "video",
            "trackID=1",
            transport,
            mpsc::channel(1).0
        );

        stream.update_statistics(1000, 90000, 1000);
        stream.update_statistics(1002, 90180, 1000); // One packet lost

        let ssrc = 0x12345678;
        let rtcp_packet = stream.generate_rtcp_report(ssrc);

        if let RTCPPacket::ReceiverReport { ssrc: pkt_ssrc, reports } = rtcp_packet {
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