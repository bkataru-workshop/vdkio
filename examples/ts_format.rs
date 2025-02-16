use bytes::Bytes;
use tokio::fs::File as AsyncFile;
use tokio::io::{BufReader, BufWriter};
use vdkio::av::{CodecData, CodecType, Packet};
use vdkio::format::ts::{TSDemuxer, TSMuxer};
use vdkio::format::{Demuxer, Muxer};

#[derive(Clone)]
struct DummyCodecData {
    codec_type: CodecType,
}

impl CodecData for DummyCodecData {
    fn codec_type(&self) -> CodecType {
        self.codec_type
    }

    fn width(&self) -> Option<u32> {
        None
    }

    fn height(&self) -> Option<u32> {
        None
    }

    fn extra_data(&self) -> Option<&[u8]> {
        None
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example: Create a TS file with dummy video and audio streams
    println!("Creating TS file with dummy streams...");

    let output_file = AsyncFile::create("output.ts").await?;
    let writer = BufWriter::new(output_file);
    let mut muxer = Box::new(TSMuxer::new(writer)) as Box<dyn Muxer>;

    // Define streams
    let streams = vec![
        Box::new(DummyCodecData {
            codec_type: CodecType::H264,
        }) as Box<dyn CodecData>,
        Box::new(DummyCodecData {
            codec_type: CodecType::AAC,
        }) as Box<dyn CodecData>,
    ];

    // Write header and initialize streams
    muxer.write_header(&streams).await?;

    // Create and write dummy packets
    for i in 0..10 {
        // Write video packet
        let video_packet = Packet::new(Bytes::from(vec![0u8; 1024]))
            .with_stream_index(0)
            .with_pts(i * 3600)
            .with_key_flag(i % 5 == 0);
        muxer.write_packet(&video_packet).await?;

        // Write audio packet
        let audio_packet = Packet::new(Bytes::from(vec![0u8; 256]))
            .with_stream_index(1)
            .with_pts(i * 1200);
        muxer.write_packet(&audio_packet).await?;
    }

    // Write trailer and finish muxing
    muxer.write_trailer().await?;
    println!("TS file created successfully!");

    // Now read back the file
    println!("\nReading back the TS file...");
    let input_file = AsyncFile::open("output.ts").await?;
    let reader = BufReader::new(input_file);
    let mut demuxer = Box::new(TSDemuxer::new(reader)) as Box<dyn Demuxer>;

    // Get stream information
    let streams = demuxer.streams().await?;
    println!("Found {} streams:", streams.len());
    for (i, stream) in streams.iter().enumerate() {
        println!("Stream {}: {:?}", i, stream.codec_type());
    }

    // Read and display some packets
    println!("\nReading packets:");
    for _ in 0..5 {
        let packet = demuxer.read_packet().await?;
        println!(
            "Packet: stream_index={}, pts={:?}, is_key={}",
            packet.stream_index, packet.pts, packet.is_key
        );
    }

    Ok(())
}
