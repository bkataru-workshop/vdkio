use bytes::Bytes;
use tokio::fs::File as AsyncFile;
use tokio::io::{AsyncReadExt, AsyncSeekExt, BufReader, BufWriter};
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
    let output_path = "./test_output/output.ts";
    // Example: Create a TS file with dummy video and audio streams
    println!("Creating TS file with dummy streams...");
    println!("Will write to: {}", output_path);

    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all("./test_output").await?;
    
    let output_file = AsyncFile::create(output_path).await?;
    let writer = BufWriter::with_capacity(8192, output_file); // Use larger buffer
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
        let mut video_data = Vec::with_capacity(1024);
        // Add PES header
        video_data.extend_from_slice(&[
            0x00, 0x00, 0x01, 0xE0, // PES start code + stream ID (video)
            0x00, 0x00, // PES packet length (will be filled later)
            0x80, // Flags
            0x80, // PTS flag set
            0x05, // PES header length
        ]);
        // Add dummy H.264 NAL unit
        video_data.extend_from_slice(&[0x00, 0x00, 0x01, 0x09]); // Access unit delimiter
        video_data.extend_from_slice(&vec![0x00; 184]); // Dummy video data

        // Calculate PES packet length
        let length = (video_data.len() - 6) as u16;
        video_data[4] = (length >> 8) as u8;
        video_data[5] = (length & 0xFF) as u8;

        let video_packet = Packet::new(Bytes::from(video_data))
            .with_stream_index(0)
            .with_pts(i * 3600)
            .with_key_flag(i % 5 == 0);
        muxer.write_packet(&video_packet).await?;

        // Write audio packet
        let mut audio_data = Vec::with_capacity(256);
        // Add PES header
        audio_data.extend_from_slice(&[
            0x00, 0x00, 0x01, 0xC0, // PES start code + stream ID (audio)
            0x00, 0x00, // PES packet length (will be filled later)
            0x80, // Flags
            0x80, // PTS flag set
            0x05, // PES header length
        ]);
        // Add dummy AAC frame
        audio_data.extend_from_slice(&[0xFF, 0xF1]); // ADTS header
        audio_data.extend_from_slice(&vec![0x00; 128]); // Dummy audio data

        // Calculate PES packet length
        let length = (audio_data.len() - 6) as u16;
        audio_data[4] = (length >> 8) as u8;
        audio_data[5] = (length & 0xFF) as u8;

        let audio_packet = Packet::new(Bytes::from(audio_data))
            .with_stream_index(1)
            .with_pts(i * 1200);
        muxer.write_packet(&audio_packet).await?;
    }

    // Write trailer and finish muxing
    muxer.write_trailer().await?;
    muxer.flush().await?;
    println!("TS file created successfully at {}", output_path);

    // Drop muxer to ensure file is closed
    drop(muxer);

    // Now read back the file
    println!("\nReading back the TS file...");
    let mut input_file = AsyncFile::open(output_path).await?;
    
    // Read and verify first few bytes of the file
    let mut header = vec![0u8; 4];
    input_file.read_exact(&mut header).await?;
    println!("First 4 bytes: {:02x} {:02x} {:02x} {:02x}", header[0], header[1], header[2], header[3]);
    
    if header[0] != 0x47 {
        println!("WARNING: First byte is not sync byte 0x47!");
    }
    
    // Rewind file for demuxer
    input_file.seek(std::io::SeekFrom::Start(0)).await?;
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
