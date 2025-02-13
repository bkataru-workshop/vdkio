use bytes::Bytes;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Packet {
    pub data: Bytes,
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub stream_index: usize,
    pub is_key: bool,
    pub duration: Option<Duration>,
}

impl Packet {
    pub fn new(data: impl Into<Bytes>) -> Self {
        Self {
            data: data.into(),
            pts: None,
            dts: None,
            stream_index: 0,
            is_key: false,
            duration: None,
        }
    }

    pub fn with_pts(mut self, pts: i64) -> Self {
        self.pts = Some(pts);
        self
    }

    pub fn with_dts(mut self, dts: i64) -> Self {
        self.dts = Some(dts);
        self
    }

    pub fn with_stream_index(mut self, index: usize) -> Self {
        self.stream_index = index;
        self
    }

    pub fn with_key_flag(mut self, is_key: bool) -> Self {
        self.is_key = is_key;
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}
