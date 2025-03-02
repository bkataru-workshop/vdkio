use std::error::Error;
use std::time::Duration;
use tokio::fs::File;
use vdkio::av::{CodecData, CodecDataExt, CodecType, Demuxer, Muxer, Packet};
use vdkio::format::aac::{AACDemuxer, AACMuxer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Example 1: Writing AAC file
    {
        // Create AAC codec configuration
        #[derive(Clone)]
        struct AACCodecData {
            codec_type: CodecType,
        }

        impl CodecData for AACCodecData {
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

        let output_file = File::create("output.aac").await?;
        let mut muxer = AACMuxer::new(output_file);

        // Create AAC stream configuration
        let aac_config = Box::new(AACCodecData {
            codec_type: CodecType::AAC,
        }) as Box<dyn CodecDataExt>;

        // Write header with AAC stream config
        muxer.write_header(&[aac_config]).await?;

        // Write some AAC frames (in this example we're just writing dummy frames)
        for i in 0..10 {
            // In a real application, these would be real AAC frames
            let dummy_adts_header = vec![
                0xFF, 0xF1, // Sync word + MPEG-4 + Layer 0 + Protection absent
                0x50, // Profile (AAC LC) + Sampling freq (44.1kHz)
                0x80, // Channel config (2 channels/stereo)
                0x43, 0x80, // Frame length (1024 samples)
                0x00, // Buffer fullness + Number of raw blocks
            ];

            let mut frame_data = dummy_adts_header;
            frame_data.extend_from_slice(&[0u8; 1024]); // Dummy AAC frame data

            let packet = Packet::new(frame_data)
                .with_stream_index(0)  // Use stream index 0 since we only have one stream
                .with_pts(i * 23_220) // 23.22ms per frame at 44.1kHz
                .with_duration(Duration::from_micros(23220))
                .with_key_flag(true);

            muxer.write_packet(packet).await?;
        }

        muxer.write_trailer().await?;
    }

    // Example 2: Reading AAC file
    {
        let input_file = File::open("output.aac").await?;
        let mut demuxer = AACDemuxer::new(input_file);

        // Get stream information
        let streams = demuxer.streams().await?;
        println!("Found {} streams", streams.len());

        // Read all packets
        while let Ok(packet) = demuxer.read_packet().await {
            println!(
                "Read AAC packet: pts={:?}, duration={:?}, size={}",
                packet.pts,
                packet.duration,
                packet.data.len()
            );
        }
    }

    Ok(())
}
