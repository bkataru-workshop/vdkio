# vdkio - Rust Video Development Kit (VDK)

A toolkit for building video streaming applications in Rust. The VDK provides a collection of modules for handling various video formats, codecs, and streaming protocols.

## Features

- Video codec support:
  - H.264/AVC parsing and frame extraction
  - AAC audio parsing and frame extraction

- Streaming protocols:
  - RTSP client implementation with SDP parsing
  - More protocols coming soon...

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
vdkio = "0.1.0"
```

### Quick Start

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
    if let Some(_video) = sdp.get_media("video") {
        client.setup("video").await?;
    }
    
    // Start playback
    client.play().await?;

    Ok(())
}
```

### Using the H.264 Parser

```rust
use vdkio::prelude::*;

fn parse_h264_frame(data: &[u8]) {
    let mut parser = H264Parser::new();
    let nalu = parser.parse_nalu(data).unwrap();
    
    if nalu.is_keyframe() {
        println!("Found keyframe!");
    }
}
```

### Using the AAC Parser

```rust
use vdkio::prelude::*;

fn parse_aac_frame(data: &[u8]) {
    let mut parser = AACParser::new();
    let frame = parser.parse_frame(data).unwrap();
    
    println!("Parsed AAC frame with {} channels", frame.config.channel_configuration);
}
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
