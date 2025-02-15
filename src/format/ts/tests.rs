// Removed unused imports
// use super::*;
// use std::io::Cursor;
// use tokio::runtime::Runtime;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::runtime::Runtime;

    // Test methods and implementations remain unchanged
    // ...

    #[test]
    fn test_ts_muxer() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let data = Vec::new();
            let mut muxer = TSMuxer::new(Cursor::new(data));

            // Add a stream and write header
            muxer.add_stream(Box::new(TestCodec)).await.unwrap();
            
            let streams = vec![Box::new(TestCodec) as Box<dyn CodecData>];
            muxer.write_header(&streams).await.unwrap();

            // Create a dummy packet with timing
            let mut packet = Packet::new(vec![0; 184]); // TS payload size (188 - 4)
            packet.stream_index = 0;
            packet.pts = Some(0);
            packet.dts = Some(0);
            muxer.write_packet(packet).await.unwrap();

            // Write trailer
            muxer.write_trailer().await.unwrap();
        });
    }

    // Other test functions remain unchanged
}