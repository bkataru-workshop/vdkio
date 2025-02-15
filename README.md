# vdkio - Rust Video Development Kit (VDK)

A toolkit for building video streaming applications in Rust. The VDK provides a collection of modules for handling various video formats, codecs, and streaming protocols. It aims to provide a high-performance, flexible, and extensible framework for video processing and streaming.

## Features

- Video codec support:
  - H.264/AVC parsing and frame extraction
  - H.265/HEVC parsing and frame extraction 
  - AAC audio parsing and frame extraction

- Streaming protocols:
  - RTSP client implementation with SDP parsing
  - RTP packet handling and media transport
  - RTCP feedback and statistics
  - TS (Transport Stream) format support
  - HLS output with segmentation

## Current State of the Project

### Fully Tested Implementations

These features have been thoroughly tested with live streams and are production-ready:

1. **Streaming Protocols:**
   - **RTSP:** Fully implemented and tested with live RTSP streams, including core operations, authentication, and SDP parsing.
   - **RTP:** Fully tested with live streams, implementing packet handling, sequence number management, and jitter buffer.
   - **RTCP:** Thoroughly tested with reception reports and error handling.

2. **Codec Support (via RTSP streams):**
   - **H.264:** Fully implemented and tested with live RTSP streams, including detailed handling of NALUs, SPS, and PPS.
   - **H.265:** Basic functionality tested with live RTSP streams; complete parameter set handling is in progress.
   - **AAC:** AAC codec parsing has been tested only as part of live RTSP streams for audio extraction. Full AAC format support remains in development.

### In Progress Implementations
The following features are implemented but require additional testing and refinement:

1. **Transport Stream (TS) Format:**
   - Core packet structure implementation ✅
   - Program Specific Information (PSI) tables ✅
   - PAT/PMT parsing and generation ✅
   - Stream type identification (H.264, H.265, AAC) ✅
   - Adaptation field support ✅
   - PCR handling ✅
   - PES packet handling ✅
   - Advanced PCR timing mechanisms ✅
    - PCR discontinuity handling ✅
    
2. **HLS Support:**
    - Segment generation and management ✅
    - Multi-bitrate streaming support ✅
    - Master/variant playlist generation ✅
    - Segment duration control ✅
   - RTSP to HLS conversion with transcoding ✅
   - Testing Status:
     - Basic RTSP to HLS conversion tested ✅
     - Multi-bitrate adaptation tested ✅
      - PCR timing accuracy tested ✅
    - Needs:
      - Testing with different codec combinations
     - Enhanced error recovery
 mechanisms
     - More comprehensive live testing

### Preliminary Implementations

The following features have preliminary implementations with basic unit tests but require thorough testing with live streams for production use:

1. **Format Support:**
   - **AAC Format:** Basic implementation of AAC file format support (muxer/demuxer) with unit tests only. Needs live stream testing.
   - **TS Format:** Core implementation complete with RTSP to HLS conversion support. Further testing needed with different codec combinations.

### Critical Next Steps

1. **Transport Stream (TS):**
   - Fine-tune PCR timing accuracy
    - Enhance error recovery mechanisms
    - Test with diverse stream types

2. **HLS Support:**
   - Basic segmentation ✅
   - Basic playlist generation ✅
   - Segment duration handling ✅
    - Multi-bitrate streaming ✅
   - Needs:
     - More comprehensive testing with varied streams
     - Multi-bitrate (adaptive) streaming
     - Enhanced error handling

### Other Planned Implementations

The following formats from `vdk` are planned for implementation. Note that live testing URLs will be required to properly validate these implementations:

1. **DVRIP** - Preliminary implementation planned
2. **FLV** - Preliminary implementation planned
3. **FMP4** - Preliminary implementation planned
4. **MKV** - Preliminary implementation planned
5. **MP4** - Preliminary implementation planned
6. **MP4F** - Preliminary implementation planned
7. **MP4M** - Preliminary implementation planned
8. **MSE** - Preliminary implementation planned
9. **NVR** - Preliminary implementation planned
10. **RAW** - Preliminary implementation planned
11. **RTMP** - Preliminary implementation planned
12. **WebRTC** - Preliminary implementation planned

Note: For these formats, basic implementations will be provided with unit tests, but full feature parity and production readiness will require testing with live streams.

### Implementation Status Summary

| Feature | vdkio Status | vdk Status | Live Testing Status |
|---------|--------------|------------|-------------------|
| RTSP | ✅ Full | ✅ Full | ✅ Tested with live streams |
| RTP | ✅ Full | ✅ Full | ✅ Tested with live streams |
| RTCP | ✅ Full | ✅ Full | ✅ Tested with live streams |
| H.264 | ✅ Full | ✅ Full | ✅ Tested with live streams |
| H.265 | ⚠️ Basic | ✅ Full | ✅ Tested with live streams |
| AAC Codec | ⚠️ Basic | ✅ Full | ⚠️ Tested via RTSP only |
| AAC Format | ⚠️ Basic | ✅ Full | ❌ Unit tests only |
| TS Format | ✅ Full | ✅ Full | ✅ Tested with RTSP conversion |
| HLS | ✅ Basic | ✅ Full | ✅ Multi-bitrate conversion tested |
| Other Formats | 🚧 Planned | ✅ Full | ❌ Awaiting test streams |

Legend:
- ✅ Full: Complete implementation
- ⚠️ Basic: Preliminary implementation
- 🚧 In Progress/Planned: Implementation in progress or planned
- ❌ Missing: Not yet implemented or no test streams available

### Testing Requirements

For thorough testing and feature parity validation, we need:

1. Live testing URLs for format support validation:
   - DVRIP
   - FLV
   - FMP4
   - MKV
   - MP4 variants
   - RTMP
   - WebRTC

2. Additional testing requirements:
   - Sample files for different codecs and formats
   - Test streams with various codec combinations
   - Infrastructure for automated testing

### Acknowledgments

- [vdk](https://github.com/deepch/vdk) for being the original, reference implementation in Go that was used.
- [Exponential-Golomb coding](https://en.wikipedia.org/wiki/Exponential-Golomb_coding) for the theory behind H.264/H.265 transcoding.
- [exp-golomb](https://crates.io/crates/exp-golomb) for providing a working, reference implementation for Exponential Golomb coding.

Built with the help of 

- 🤖 Gemini 2.0
- 🤖 Claude 3.5 Sonnet
- 🤖 OpenAI GPT-4o-mini
- 🚀 Roo Code
- 🚀 Cline

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
vdkio = "0.1.0"
```

## Examples

See the `examples/` directory:

- `rtsp_player.rs`: Working with RTSP streams
- `ts_format.rs`: Working with Transport Stream format
- `aac_format.rs`: Working with AAC files (basic implementation)
- `rtsp_to_hls.rs`: Converting RTSP streams to HLS format
- More examples will be added as new formats are implemented

## Usage Notes

1. **RTSP Streams:**
   - Use RTSPClient for connecting to and consuming RTSP streams
   - Supports authentication and connection management
   - Includes setup options for video/audio selection

2. **HLS Streaming:**
   - Supports conversion from RTSP to HLS
   - Provides segmentation and playlist management
   - Configurable segment duration and retention
    - Features:
      - Multi-bitrate adaptation support
      - Automatic segment cleanup
      - PCR timing accuracy
      - Error recovery with reconnection

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
