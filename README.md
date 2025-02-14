# vdkio - Rust Video Development Kit (VDK)

A toolkit for building video streaming applications in Rust. The VDK provides a collection of modules for handling various video formats, codecs, and streaming protocols.

## Features

- Video codec support:
  - H.264/AVC parsing and frame extraction
  - H.265/HEVC parsing and frame extraction 
  - AAC audio parsing and frame extraction

- Streaming protocols:
  - RTSP client implementation with SDP parsing
  - RTP packet handling and media transport
  - RTCP feedback and statistics

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
vdkio = "0.1.0"
```

## Quick Start

```rust
use vdkio::prelude::*;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create an RTSP client and connect to a stream
    let mut client = RTSPClient::new("rtsp://example.com/stream")?;
    client.connect().await?;
    
    // Get stream information
    let sdp = client.describe().await?;
    
    // Set up video stream if available
    if let Some(video) = sdp.get_media("video") {
        client.setup("video").await?;
    }
    
    // Start playback
    client.play().await?;

    Ok(())
}
```

## H.264 Parser Example

```rust
use vdkio::prelude::*;
use std::error::Error;

fn parse_h264_frame(data: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut parser = H264Parser::new();
    let nalu = parser.parse_nalu(data)?;
    
    if nalu.is_keyframe() {
        println!("Found keyframe!");
    }

    Ok(())
}
```

## AAC Parser Example

```rust
use vdkio::prelude::*;
use std::error::Error;

fn parse_aac_frame(data: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut parser = AACParser::new();
    let frame = parser.parse_frame(data)?;
    
    println!("Parsed AAC frame with {} channels", frame.config.channel_configuration);

    Ok(())
}
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
