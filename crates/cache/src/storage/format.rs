//! Storage format definitions and header structures
//!
//! This module defines the binary format used for cache storage,
//! including magic numbers, versioning, and checksums.

use crate::errors::{CacheError, RecoveryHint, Result};
use crc32c::crc32c;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Magic number for cache files: "CUEV" (CUEnV cache)
pub const CACHE_MAGIC: u32 = 0x43554556;

/// Current storage format version
pub const STORAGE_VERSION: u16 = 2;

/// Default zstd compression level (3 = fast with good compression)
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3;

/// Maximum WAL size before rotation (10MB)
pub const MAX_WAL_SIZE: u64 = 10 * 1024 * 1024;

/// Binary storage header for all cache files
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C)]
pub struct StorageHeader {
    /// Magic number for validation
    magic: u32,
    /// Storage format version
    version: u16,
    /// Flags (bit 0: compressed, bit 1: encrypted, etc.)
    flags: u16,
    /// CRC32C of the header (excluding this field)
    header_crc: u32,
    /// Timestamp when written
    timestamp: u64,
    /// Uncompressed data size
    uncompressed_size: u64,
    /// Compressed data size (same as uncompressed if not compressed)
    compressed_size: u64,
    /// CRC32C of the data payload
    pub data_crc: u32,
    /// Reserved for future use
    reserved: [u8; 16],
}

impl StorageHeader {
    const FLAG_COMPRESSED: u16 = 1 << 0;
    const _FLAG_ENCRYPTED: u16 = 1 << 1;

    pub fn new(
        uncompressed_size: u64,
        compressed_size: u64,
        data_crc: u32,
        compressed: bool,
    ) -> Self {
        let mut header = Self {
            magic: CACHE_MAGIC,
            version: STORAGE_VERSION,
            flags: if compressed { Self::FLAG_COMPRESSED } else { 0 },
            header_crc: 0,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            uncompressed_size,
            compressed_size,
            data_crc,
            reserved: [0u8; 16],
        };

        // Calculate header CRC (excluding the CRC field itself)
        header.header_crc = header.calculate_crc();
        header
    }

    fn calculate_crc(&self) -> u32 {
        // Serialize header with CRC field set to 0
        let mut temp = *self;
        temp.header_crc = 0;

        let bytes = match bincode::serialize(&temp) {
            Ok(b) => b,
            Err(_) => return 0,
        };

        crc32c(&bytes)
    }

    pub fn validate(&self) -> Result<()> {
        // Check magic number
        if self.magic != CACHE_MAGIC {
            return Err(CacheError::Corruption {
                key: String::new(),
                reason: format!(
                    "Invalid magic number: expected {:08x}, got {:08x}",
                    CACHE_MAGIC, self.magic
                ),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }

        // Check version
        if self.version > STORAGE_VERSION {
            return Err(CacheError::Corruption {
                key: String::new(),
                reason: format!("Unsupported storage version: {}", self.version),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Update cuenv to support newer cache format".to_string(),
                },
            });
        }

        // Verify header CRC
        let expected_crc = self.calculate_crc();
        if self.header_crc != expected_crc {
            return Err(CacheError::Corruption {
                key: String::new(),
                reason: format!(
                    "Header CRC mismatch: expected {:08x}, got {:08x}",
                    expected_crc, self.header_crc
                ),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }

        Ok(())
    }

    pub fn is_compressed(&self) -> bool {
        self.flags & Self::FLAG_COMPRESSED != 0
    }
}
