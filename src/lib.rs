pub mod av;
pub mod codec;
pub mod error;
pub mod format;
pub mod utils;

pub use error::{Result, VdkError};

// Re-export common types for convenience
pub use av::CodecType;
pub use format::rtp::{RTPPacket, JitterBuffer};
pub use format::rtcp::{RTCPPacket, ReceptionReport};
pub use format::rtsp::{RTSPClient, MediaDescription, TransportInfo, CastType, StreamStatistics};

#[cfg(test)]
mod tests {
    

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
