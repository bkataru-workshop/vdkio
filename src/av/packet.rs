use bytes::Bytes;
use std::time::Duration;

/// Represents a media packet containing encoded audio or video data.
///
/// A `Packet` is the basic unit of media data in the vdkio system. It contains
/// the actual media data along with timing information (PTS/DTS), stream identification,
/// and other metadata necessary for proper media handling.
#[derive(Debug, Clone)]
pub struct Packet {
    /// The actual media data contained in the packet
    pub data: Bytes,
    /// Presentation Time Stamp (PTS) in the media's time base
    pub pts: Option<i64>,
    /// Decoding Time Stamp (DTS) in the media's time base
    pub dts: Option<i64>,
    /// Index of the stream this packet belongs to
    pub stream_index: usize,
    /// Indicates whether this packet contains a key frame
    pub is_key: bool,
    /// Duration of the media content in this packet
    pub duration: Option<Duration>,
}

impl Packet {
    /// Creates a new media packet with the given data.
    ///
    /// All timing and metadata fields are initialized to their default values:
    /// - No PTS/DTS timestamps
    /// - Stream index of 0
    /// - Not marked as a key frame
    /// - No duration set
    ///
    /// # Arguments
    ///
    /// * `data` - The media data to store in the packet. Can be anything that can be
    ///            converted into `Bytes`.
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

    /// Sets the Presentation Time Stamp (PTS) for this packet.
    ///
    /// # Arguments
    ///
    /// * `pts` - The presentation timestamp value in the media's time base
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    pub fn with_pts(mut self, pts: i64) -> Self {
        self.pts = Some(pts);
        self
    }

    /// Sets the Decoding Time Stamp (DTS) for this packet.
    ///
    /// # Arguments
    ///
    /// * `dts` - The decoding timestamp value in the media's time base
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    pub fn with_dts(mut self, dts: i64) -> Self {
        self.dts = Some(dts);
        self
    }

    /// Sets the stream index for this packet.
    ///
    /// The stream index identifies which elementary stream this packet belongs to
    /// within a container format (e.g., stream 0 might be video, stream 1 might be audio).
    ///
    /// # Arguments
    ///
    /// * `index` - The zero-based index of the stream this packet belongs to
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    pub fn with_stream_index(mut self, index: usize) -> Self {
        self.stream_index = index;
        self
    }

    /// Sets whether this packet contains a key frame.
    ///
    /// Key frames (also known as I-frames in video) are frames that can be decoded
    /// independently of other frames. This is important for seeking and random access
    /// in media streams.
    ///
    /// # Arguments
    ///
    /// * `is_key` - True if this packet contains a key frame, false otherwise
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    pub fn with_key_flag(mut self, is_key: bool) -> Self {
        self.is_key = is_key;
        self
    }

    /// Sets the duration of the media content in this packet.
    ///
    /// # Arguments
    ///
    /// * `duration` - The duration of the media data contained in this packet
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}
