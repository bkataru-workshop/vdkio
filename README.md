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
   - **H.265:** Basic functionality tested with live RTSP streams. Complete parameter set handling and more extensive testing are in progress.
   - **AAC:** AAC codec parsing has been tested as part of live RTSP streams for audio extraction, confirming basic functionality. Full AAC format support and more comprehensive testing remain in development. 

### In Progress Implementations
The following features are implemented and are undergoing further testing and refinement to ensure stability and production readiness:

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
    - **Basic RTSP to HLS Conversion:** Implemented and basic integration testing completed ✅. Provides a functional RTSP to HLS conversion pipeline.
    - Segment generation and management ✅
    - Basic playlist generation and management ✅
    - Segment duration control ✅
    - Multi-bitrate streaming support ✅
   - RTSP to HLS conversion with transcoding ✅
   - Testing Status:
     - Basic RTSP to HLS conversion tested ✅
     - Multi-bitrate adaptation tested ✅
     - Playlist generation and segment management tested ✅
     - Basic integration test now passing ✅
      - PCR timing accuracy tested ✅
    - Needs:
      - **Comprehensive Testing:** More comprehensive testing with varied RTSP streams and codec combinations is needed to ensure robustness and identify potential issues.
     - Enhanced error recovery and handling mechanisms
     - Further refinement and optimization for performance and stability
     - More comprehensive live testing for production readiness
   

### Preliminary Implementations

The following features have preliminary implementations with basic unit tests. These implementations are functional but require thorough testing with live streams and more comprehensive tests for production use. 

**Note:** For formats listed under "Preliminary Implementations" and "Other Planned Implementations", live testing URLs are required for full validation and feature parity. Basic implementations and unit tests are provided, but comprehensive testing with live streams is essential for production readiness.

1. **Format Support:**
   - **AAC Format:** Basic implementation of AAC file format support (muxer/demuxer) with unit tests only. Needs live stream testing and more comprehensive tests.
   - **TS Format:** Core TS format implementation is complete and used in RTSP to HLS conversion. Further testing needed with different codec combinations and transport stream variations.

### Critical Next Steps

1. **Comprehensive Testing:**
   - **HLS Support:** Conduct more comprehensive testing with varied RTSP streams and codec combinations to ensure the robustness of HLS streaming and identify any potential issues.
   - **TS Format:** Perform thorough testing with diverse stream types and transport stream variations to fully validate TS format handling and error recovery.

2. **Enhanced Error Handling and Recovery:**
   - **HLS Support:** Implement more robust error recovery and handling mechanisms for HLS streaming to improve stability and reliability in real-world scenarios.
   - **TS Format:** Enhance error recovery mechanisms for TS format handling to ensure robust and fault-tolerant stream processing.

3. **Performance Optimization:**
   - **General:** Identify and address any performance bottlenecks in TS muxing and HLS segmenting to optimize vdkio for high-performance video streaming applications.

4. **Acquire Live Testing URLs**:
   - Obtain live testing URLs for the following formats to enable full feature validation, comprehensive testing, and ensure feature parity with the reference vdk implementation:
     - DVRIP
     - FLV
     - FMP4
     - MKV
     - MP4 variants (MP4, MP4F, MP4M)
     - RTMP
     - WebRTC


### Other Planned Implementations

The following formats from `vdk` are planned for implementation.  Live testing URLs will be essential to properly validate these implementations and ensure production readiness:

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
| HLS | ⚠️ Basic | ✅ Full | ⚠️ Basic RTSP to HLS conversion tested; more testing needed |
| Other Formats | 🚧 Planned | ✅ Full | ❌ Awaiting test streams |

Legend:
- ✅ Full: Complete implementation
- ⚠️ Basic: Preliminary implementation
- 🚧 In Progress/Planned: Implementation in progress or planned
- ❌ Missing: Not yet implemented or no test streams available

### Testing Requirements

For thorough testing and feature parity validation, the following are needed:

1. **Live testing URLs for format support validation**: Essential for validating format implementations and achieving feature parity.
   - DVRIP
   - FLV
   - FMP4
   - MKV
   - MP4 variants
   - RTMP
   - WebRTC

2. **Additional testing requirements**:
   - Sample files for different codecs and formats to expand test coverage.
   - Test streams with various codec combinations to ensure compatibility and robustness.
   - Infrastructure for automated testing to streamline testing and ensure consistent validation.

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
