# Rust VDK Implementation Plan

## 1. Project Setup and Structure

### 1.1 Project Layout

```
rust-vdk/
├── Cargo.toml
├── src/
│   ├── av/
│   │   ├── mod.rs           # Core traits and types
│   │   ├── codec.rs         # Codec interfaces
│   │   ├── packet.rs        # Packet handling
│   │   └── format.rs        # Format traits
│   ├── codec/
│   │   ├── mod.rs
│   │   ├── h264/
│   │   ├── h265/
│   │   ├── aac/
│   │   └── rtmp/
│   ├── format/
│   │   ├── mod.rs
│   │   ├── rtsp/
│   │   ├── rtp/
│   │   ├── rtcp/
│   │   └── mp4/
│   └── utils/
      ├── mod.rs
      ├── bits.rs
      └── buffers.rs
```

### 1.2 Core Dependencies

```toml
[package]
name = "vdkio"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
bytes = "1"
futures = "0.3"
thiserror = "1"
async-trait = "0.1"
parking_lot = "0.12"
bitvec = "1"
log = "0.4"
```

## 2. Core Traits and Types

### 2.1 Base Traits

```rust
pub enum CodecType {
    H264,
    H265,
    AAC,
    OPUS,
}

#[async_trait]
pub trait CodecData: Send + Sync {
    fn codec_type(&self) -> CodecType;
    fn width(&self) -> Option<u32>;
    fn height(&self) -> Option<u32>;
    fn extra_data(&self) -> Option<&[u8]>;
}

#[async_trait]
pub trait Demuxer: Send {
    async fn read_packet(&mut self) -> Result<Packet>;
    async fn streams(&mut self) -> Result<Vec<Box<dyn CodecData>>>;
}

#[async_trait]
pub trait Muxer: Send {
    async fn write_header(&mut self, streams: &[Box<dyn CodecData>]) -> Result<()>;
    async fn write_packet(&mut self, packet: Packet) -> Result<()>;
    async fn write_trailer(&mut self) -> Result<()>;
}
```

## 3. Feature Status

**Currently Implemented:**
1. Core Infrastructure
   - ✅ Base traits and types
   - ✅ Error handling system
   - ✅ Bit parsing utilities
   - ✅ Basic test framework

2. Codec Support
   - ✅ H264 Parser (basic functionality)
   - 🟡 H265 Parser (in progress)
     * ✅ NAL unit parsing
     * ✅ SPS parsing
     * ✅ Basic PPS parsing
     * ✅ Basic VPS parsing
     * ❌ Complete parameter set handling
   - ✅ AAC Parser (basic functionality)

3. Format Support
   - ✅ RTSP Client
     * ✅ Core RTSP operations
     * ✅ Authentication (Basic, Digest)
     * ✅ SDP parsing
     * ✅ UDP RTP reception
     * ✅ RTCP support
     * ✅ Jitter buffer implementation
   - ✅ RTP/RTCP Implementation
     * ✅ RTP packet handling
     * ✅ RTCP packet types
     * ✅ Jitter buffer
     * ✅ Statistics tracking

**Missing Features:**
1. Codec Support
   - Complete H265 implementation
     * Reference picture management
     * Performance optimization
     * Comprehensive testing
   - MJPEG Parser
   - OPUS Parser
   - Hardware acceleration

2. Format Support
   - RTMP Client/Server
   - WebRTC stack
   - HLS Muxer
   - MP4/FMP4 Container
   - DVRIP Protocol
   - Transport Stream (TS)

3. Advanced Features
   - Transcoding
   - Pub/Sub system
   - Packet queueing
   - FFmpeg integration

## 4. Implementation Priorities

1. Complete H265 Implementation (Highest Priority)
   - Complete parameter set parsing (SPS/PPS/VPS)
   - Frame assembly
   - Reference picture handling
   - Performance optimization

2. Container Format Support
   - MP4/FMP4 implementation
   - WebRTC transport layer
   - Basic RTMP client

3. Additional Codec Support
   - MJPEG parser implementation
   - OPUS parser
   - Hardware acceleration support

## 5. Next Steps

1. Complete H265 support
   - Complete parameter set parsing
   - Frame assembly
   - Unit tests
   - Performance testing

2. Begin container format support
   - MP4 demuxer implementation
   - Fragment support
   - Streaming optimization

3. Start MJPEG implementation
   - JPEG parsing
   - RTP payload format
   - Integration with RTSP
