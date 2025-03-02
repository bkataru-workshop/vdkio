use crate::error::{Result, VdkError};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncWrite, AsyncWriteExt};

const DEFAULT_SEGMENT_DURATION: Duration = Duration::from_secs(2);
const DEFAULT_PLAYLIST_SIZE: usize = 5;

/// Represents a variant stream in an HLS master playlist.
///
/// HLS supports multiple variant streams with different qualities and bitrates,
/// allowing clients to switch between them based on network conditions.
#[derive(Debug, Clone)]
pub struct HLSVariant {
    /// Unique identifier for this variant
    pub name: String,
    /// Average bandwidth in bits per second
    pub bandwidth: u32,
    /// Video resolution as (width, height) if applicable
    pub resolution: Option<(u32, u32)>,
    /// RFC 6381 codec string (e.g., "avc1.64001f,mp4a.40.2")
    pub codecs: String,
}

/// Represents a media segment in an HLS playlist.
///
/// Each segment contains a portion of the media stream and has associated
/// timing and sequence information.
#[derive(Debug)]
pub struct HLSSegment {
    /// Name of the segment file
    pub filename: String,
    /// Duration of media content in the segment
    pub duration: Duration,
    /// Monotonically increasing sequence number
    pub sequence_number: u32,
    /// Optional byte range for partial segments
    pub byte_range: Option<(u64, u64)>,
}

/// Represents an HLS media playlist (*.m3u8).
///
/// A media playlist contains information about media segments and their ordering,
/// along with timing and playback information.
#[derive(Debug)]
pub struct HLSPlaylist {
    /// HLS protocol version (usually 3)
    pub version: u8,
    /// Maximum segment duration
    pub target_duration: Duration,
    /// First sequence number in the playlist
    pub media_sequence: u32,
    /// List of media segments
    pub segments: Vec<HLSSegment>,
    /// Indicates if the playlist is complete
    pub is_endlist: bool,
    /// Optional variant information for master playlists
    pub variant: Option<HLSVariant>,
}

impl HLSPlaylist {
    /// Creates a new HLS playlist with the specified target duration.
    ///
    /// # Arguments
    ///
    /// * `target_duration` - Maximum duration of any segment in the playlist
    pub fn new(target_duration: Duration) -> Self {
        Self {
            version: 3,
            target_duration,
            media_sequence: 0,
            segments: Vec::new(),
            is_endlist: false,
            variant: None,
        }
    }

    /// Sets the variant information for this playlist.
    ///
    /// # Arguments
    ///
    /// * `variant` - The variant stream information
    pub fn with_variant(mut self, variant: HLSVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    /// Writes the playlist to an async writer in M3U8 format.
    ///
    /// # Arguments
    ///
    /// * `writer` - The writer to write the M3U8 content to
    pub async fn write_to<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        // Write basic M3U8 header
        writer.write_all(b"#EXTM3U\n").await?;
        writer.write_all(b"#EXT-X-VERSION:3\n").await?;

        // Write stream info if this is a variant playlist
        if let Some(variant) = &self.variant {
            if let Some((width, height)) = variant.resolution {
                writer
                    .write_all(
                        format!(
                            "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}x{},CODECS=\"{}\"\n",
                            variant.bandwidth, width, height, variant.codecs
                        )
                        .as_bytes(),
                    )
                    .await?;
            } else {
                writer
                    .write_all(
                        format!(
                            "#EXT-X-STREAM-INF:BANDWIDTH={},CODECS=\"{}\"\n",
                            variant.bandwidth, variant.codecs
                        )
                        .as_bytes(),
                    )
                    .await?;
            }
        }

        // Write target duration (ceiling of max segment duration)
        let max_duration = self
            .segments
            .iter()
            .map(|s| s.duration)
            .max()
            .unwrap_or(self.target_duration);

        writer
            .write_all(
                format!(
                    "#EXT-X-TARGETDURATION:{}\n",
                    max_duration.as_secs_f64().ceil() as u32
                )
                .as_bytes(),
            )
            .await?;

        // Write media sequence
        writer
            .write_all(format!("#EXT-X-MEDIA-SEQUENCE:{}\n", self.media_sequence).as_bytes())
            .await?;

        // Write segments
        for segment in &self.segments {
            writer
                .write_all(format!("#EXTINF:{:.3},\n", segment.duration.as_secs_f64()).as_bytes())
                .await?;

            if let Some((start, length)) = segment.byte_range {
                writer
                    .write_all(format!("#EXT-X-BYTERANGE:{}@{}\n", length, start).as_bytes())
                    .await?;
            }
            writer.write_all(segment.filename.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }

        // Write endlist if playlist is complete
        if self.is_endlist {
            writer.write_all(b"#EXT-X-ENDLIST\n").await?;
        }

        writer.flush().await?;
        Ok(())
    }
}

/// Represents an HLS master playlist containing multiple variant streams.
///
/// The master playlist allows clients to choose the most appropriate quality
/// level based on their network conditions and device capabilities.
#[derive(Debug)]
pub struct HLSMasterPlaylist {
    /// List of available variant streams
    pub variants: Vec<HLSVariant>,
}

impl HLSMasterPlaylist {
    /// Creates a new empty master playlist.
    pub fn new() -> Self {
        Self {
            variants: Vec::new(),
        }
    }

    /// Adds a variant stream to the master playlist.
    ///
    /// # Arguments
    ///
    /// * `variant` - The variant stream to add
    pub fn add_variant(&mut self, variant: HLSVariant) {
        self.variants.push(variant);
    }

    /// Writes the master playlist to an async writer in M3U8 format.
    ///
    /// # Arguments
    ///
    /// * `writer` - The writer to write the M3U8 content to
    pub async fn write_to<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(b"#EXTM3U\n").await?;
        writer.write_all(b"#EXT-X-VERSION:3\n").await?;

        for variant in &self.variants {
            if let Some((width, height)) = variant.resolution {
                writer
                    .write_all(
                        format!(
                            "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}x{},CODECS=\"{}\"\n",
                            variant.bandwidth, width, height, variant.codecs
                        )
                        .as_bytes(),
                    )
                    .await?;
            } else {
                writer
                    .write_all(
                        format!(
                            "#EXT-X-STREAM-INF:BANDWIDTH={},CODECS=\"{}\"\n",
                            variant.bandwidth, variant.codecs
                        )
                        .as_bytes(),
                    )
                    .await?;
            }
            writer
                .write_all(format!("{}.m3u8\n", variant.name).as_bytes())
                .await?;
        }

        writer.flush().await?;
        Ok(())
    }
}

/// Manages the creation and maintenance of HLS segments and playlists.
///
/// The segmenter handles creating TS segments, updating playlists, and managing
/// the sliding window of available segments.
#[derive(Debug)]
pub struct HLSSegmenter {
    /// Directory where segments and playlists are written
    output_dir: PathBuf,
    /// Target duration for each segment
    segment_duration: Duration,
    /// Maximum number of segments to keep in the playlist
    max_segments: usize,
    /// Current segment sequence number
    sequence_number: u32,
    /// Media playlist for this variant
    playlist: HLSPlaylist,
    /// Master playlist containing all variants
    master_playlist: HLSMasterPlaylist,
    /// Information about the currently active segment
    current_segment: Option<(PathBuf, Duration, u64)>,
    /// Current variant stream configuration
    variant: Option<HLSVariant>,
}

impl HLSSegmenter {
    /// Creates a new HLS segmenter writing to the specified directory.
    ///
    /// # Arguments
    ///
    /// * `output_dir` - Directory where segments and playlists will be written
    pub fn new<P: AsRef<Path>>(output_dir: P) -> Self {
        Self {
            output_dir: output_dir.as_ref().to_owned(),
            segment_duration: DEFAULT_SEGMENT_DURATION,
            max_segments: DEFAULT_PLAYLIST_SIZE,
            sequence_number: 0,
            playlist: HLSPlaylist::new(DEFAULT_SEGMENT_DURATION),
            master_playlist: HLSMasterPlaylist::new(),
            current_segment: None,
            variant: None,
        }
    }

    /// Sets the target duration for each segment.
    ///
    /// # Arguments
    ///
    /// * `duration` - The target segment duration
    pub fn with_segment_duration(mut self, duration: Duration) -> Self {
        self.segment_duration = duration;
        self.playlist.target_duration = duration;
        self
    }

    /// Sets the maximum number of segments to keep in the playlist.
    ///
    /// # Arguments
    ///
    /// * `count` - Maximum number of segments
    pub fn with_max_segments(mut self, count: usize) -> Self {
        self.max_segments = count;
        self
    }

    /// Adds a variant stream configuration.
    ///
    /// # Arguments
    ///
    /// * `variant` - The variant stream configuration
    pub fn with_variant(mut self, variant: HLSVariant) -> Self {
        self.variant = Some(variant.clone());
        self.playlist = self.playlist.with_variant(variant.clone());
        self.master_playlist.add_variant(variant);
        self
    }

    /// Starts a new segment at the specified timestamp.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The starting timestamp for the new segment
    ///
    /// # Returns
    ///
    /// A file handle for writing segment data
    pub async fn start_segment(&mut self, timestamp: Duration) -> Result<File> {
        let prefix = self
            .variant
            .as_ref()
            .map(|v| v.name.as_str())
            .unwrap_or("stream");
        let filename = format!("{}_{}.ts", prefix, self.sequence_number);
        let path = self.output_dir.join(&filename);

        let file = File::create(&path).await?;
        self.current_segment = Some((path, timestamp, 0));
        Ok(file)
    }

    /// Finishes the current segment and updates the playlist.
    ///
    /// # Arguments
    ///
    /// * `end_timestamp` - The ending timestamp for the segment
    pub async fn finish_segment(&mut self, end_timestamp: Duration) -> Result<()> {
        if let Some((path, start_time, _bytes_written)) = self.current_segment.take() {
            let duration = end_timestamp - start_time;
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| VdkError::InvalidData("Invalid segment filename".into()))?
                .to_string();

            let segment = HLSSegment {
                filename,
                duration,
                sequence_number: self.sequence_number,
                byte_range: None,
            };

            self.playlist.segments.push(segment);

            // Remove old segments if we exceed max_segments
            while self.playlist.segments.len() > self.max_segments {
                if let Some(old_segment) = self.playlist.segments.first() {
                    let old_path = self.output_dir.join(&old_segment.filename);
                    tokio::fs::remove_file(old_path).await?;
                }
                self.playlist.segments.remove(0);
                self.playlist.media_sequence += 1;
            }

            self.sequence_number += 1;
        }

        Ok(())
    }

    /// Writes the current media playlist to the provided writer.
    pub async fn write_playlist<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        self.playlist.write_to(writer).await
    }

    /// Writes the master playlist to the provided writer.
    pub async fn write_master_playlist<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        self.master_playlist.write_to(writer).await
    }

    /// Checks if a new segment should be started based on timing.
    ///
    /// # Arguments
    ///
    /// * `current_time` - The current timestamp to check against
    pub fn should_start_new_segment(&self, current_time: Duration) -> bool {
        if let Some((_, start_time, _)) = &self.current_segment {
            current_time - *start_time >= self.segment_duration
        } else {
            true
        }
    }

    /// Returns the output directory path.
    pub fn get_output_dir(&self) -> &PathBuf {
        &self.output_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::runtime::Runtime;

    #[test]
    fn test_master_playlist_generation() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut master = HLSMasterPlaylist::new();

            master.add_variant(HLSVariant {
                name: "high".to_string(),
                bandwidth: 2_000_000,
                resolution: Some((1280, 720)),
                codecs: "avc1.64001f,mp4a.40.2".to_string(),
            });

            master.add_variant(HLSVariant {
                name: "medium".to_string(),
                bandwidth: 1_000_000,
                resolution: Some((854, 480)),
                codecs: "avc1.64001f,mp4a.40.2".to_string(),
            });

            let mut buffer = Cursor::new(Vec::new());
            master.write_to(&mut buffer).await.unwrap();

            let content = String::from_utf8(buffer.into_inner()).unwrap();
            assert!(content.contains("#EXT-X-STREAM-INF:BANDWIDTH=2000000"));
            assert!(content.contains("high.m3u8"));
            assert!(content.contains("#EXT-X-STREAM-INF:BANDWIDTH=1000000"));
            assert!(content.contains("medium.m3u8"));
        });
    }

    #[test]
    fn test_variant_playlist_generation() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let variant = HLSVariant {
                name: "high".to_string(),
                bandwidth: 2_000_000,
                resolution: Some((1280, 720)),
                codecs: "avc1.64001f,mp4a.40.2".to_string(),
            };

            let playlist = HLSPlaylist::new(Duration::from_secs(2)).with_variant(variant);

            let mut buffer = Cursor::new(Vec::new());
            playlist.write_to(&mut buffer).await.unwrap();

            let content = String::from_utf8(buffer.into_inner()).unwrap();
            assert!(content.contains("#EXT-X-STREAM-INF:BANDWIDTH=2000000"));
            assert!(content.contains("RESOLUTION=1280x720"));
            assert!(content.contains("avc1.64001f,mp4a.40.2"));
        });
    }

    #[test]
    fn test_segmenter_with_variants() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let temp_dir = std::env::temp_dir();
            let variant = HLSVariant {
                name: "high".to_string(),
                bandwidth: 2_000_000,
                resolution: Some((1280, 720)),
                codecs: "avc1.64001f,mp4a.40.2".to_string(),
            };

            let mut segmenter = HLSSegmenter::new(&temp_dir)
                .with_segment_duration(Duration::from_secs(2))
                .with_max_segments(2)
                .with_variant(variant);

            for i in 0..3 {
                let start_time = Duration::from_secs(i * 2);
                let _file = segmenter.start_segment(start_time).await.unwrap();
                segmenter
                    .finish_segment(start_time + Duration::from_secs(2))
                    .await
                    .unwrap();
            }

            assert_eq!(segmenter.playlist.segments.len(), 2);
            assert_eq!(segmenter.playlist.media_sequence, 1);
            assert!(segmenter.master_playlist.variants.len() == 1);
        });
    }
}
