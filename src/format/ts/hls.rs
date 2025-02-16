use crate::error::{Result, VdkError};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncWrite, AsyncWriteExt};

const DEFAULT_SEGMENT_DURATION: Duration = Duration::from_secs(2);
const DEFAULT_PLAYLIST_SIZE: usize = 5;

#[derive(Debug, Clone)]
pub struct HLSVariant {
    pub name: String,
    pub bandwidth: u32,
    pub resolution: Option<(u32, u32)>,
    pub codecs: String,
}

#[derive(Debug)]
pub struct HLSSegment {
    pub filename: String,
    pub duration: Duration,
    pub sequence_number: u32,
    pub byte_range: Option<(u64, u64)>,
}

#[derive(Debug)]
pub struct HLSPlaylist {
    pub version: u8,
    pub target_duration: Duration,
    pub media_sequence: u32,
    pub segments: Vec<HLSSegment>,
    pub is_endlist: bool,
    pub variant: Option<HLSVariant>,
}

impl HLSPlaylist {
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

    pub fn with_variant(mut self, variant: HLSVariant) -> Self {
        self.variant = Some(variant);
        self
    }

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
            // Write segment duration
            writer
                .write_all(format!("#EXTINF:{:.3},\n", segment.duration.as_secs_f64()).as_bytes())
                .await?;

            // Write segment URI with optional byte range
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

#[derive(Debug)]
pub struct HLSMasterPlaylist {
    pub variants: Vec<HLSVariant>,
}

impl HLSMasterPlaylist {
    pub fn new() -> Self {
        Self {
            variants: Vec::new(),
        }
    }

    pub fn add_variant(&mut self, variant: HLSVariant) {
        self.variants.push(variant);
    }

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

pub struct HLSSegmenter {
    output_dir: PathBuf,
    segment_duration: Duration,
    max_segments: usize,
    sequence_number: u32,
    playlist: HLSPlaylist,
    master_playlist: HLSMasterPlaylist,
    current_segment: Option<(PathBuf, Duration, u64)>,
    variant: Option<HLSVariant>,
}

impl HLSSegmenter {
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

    pub fn with_segment_duration(mut self, duration: Duration) -> Self {
        self.segment_duration = duration;
        self.playlist.target_duration = duration;
        self
    }

    pub fn with_max_segments(mut self, count: usize) -> Self {
        self.max_segments = count;
        self
    }

    pub fn with_variant(mut self, variant: HLSVariant) -> Self {
        self.variant = Some(variant.clone());
        self.playlist = self.playlist.with_variant(variant.clone());
        self.master_playlist.add_variant(variant);
        self
    }

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

    pub async fn write_playlist<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        self.playlist.write_to(writer).await
    }

    pub async fn write_master_playlist<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        self.master_playlist.write_to(writer).await
    }

    pub fn should_start_new_segment(&self, current_time: Duration) -> bool {
        if let Some((_, start_time, _)) = &self.current_segment {
            current_time - *start_time >= self.segment_duration
        } else {
            true
        }
    }

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
