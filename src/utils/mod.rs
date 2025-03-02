//! # Utility Functions and Types
//!
//! This module provides common utility functions and types used throughout the vdkio library.
//! It includes implementations for:
//!
//! - Bit-level operations and manipulation
//! - CRC calculation and validation
//! - Stream processing utilities
//!
//! ## Bit Operations
//!
//! The bits module provides utilities for working with bit-level data:
//!
//! ```rust
//! use vdkio::utils::BitReader;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let data = vec![0b10110011u8];
//! let mut reader = BitReader::new(&data);
//!
//! // Read specific number of bits
//! let value = reader.read_bits(3)?; // Reads first 3 bits (101)
//! assert_eq!(value, 0b101);
//! # Ok(())
//! # }
//! ```
//!
//! ## CRC Calculation
//!
//! The crc module provides MPEG-2 CRC32 calculation:
//!
//! ```rust
//! use vdkio::utils::Crc32Mpeg2;
//!
//! # fn main() {
//! let data = b"Hello, world!";
//! let crc = Crc32Mpeg2::calculate(data);
//! println!("CRC32: {:08x}", crc);
//! # }
//! ```

/// Bit manipulation and bitstream reading utilities
pub mod bits;

/// CRC calculation implementations
pub mod crc;

// Re-export commonly used types
pub use bits::*;
pub use crc::Crc32Mpeg2;
