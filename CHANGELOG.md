# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-02-17

### Added
- Initial release of the vdkio library
- Core video codec support:
  - H.264/AVC parsing and frame extraction
  - H.265/HEVC basic parsing and frame extraction
  - AAC audio codec parsing via RTSP
- Streaming protocol implementations:
  - RTSP client with full protocol support
  - RTP packet handling and media transport
  - RTCP feedback and statistics
  - TS (Transport Stream) format support
  - Basic HLS output with segmentation
- Core features:
  - Packet and frame abstractions
  - Stream management
  - Basic transcoding support
  - Bitstream utilities
  - Comprehensive error handling
- Documentation:
  - Full API documentation with examples
  - Integration test coverage
  - Cross-platform CI/CD setup

### Fixed
- H.265 parser implementation and documentation
- TS muxer timing and discontinuity handling
- RTSP to HLS conversion pipeline stability

[0.1.0]: https://github.com/rust-vdk/vdkio/releases/tag/v0.1.0