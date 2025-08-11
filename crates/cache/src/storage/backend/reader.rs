//! Read operations for the storage backend
//!
//! This module handles reading data with decompression
//! and checksum verification.

use crate::errors::{CacheError, RecoveryHint, Result, SerializationOp, StoreType};
use crate::storage::format::StorageHeader;
use crc32c::crc32c;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::sync::Semaphore;
use zstd::stream::decode_all as zstd_decode;

/// Read data with decompression and checksum verification
pub async fn read_data(path: &Path, io_semaphore: &Semaphore) -> Result<Vec<u8>> {
    let _permit = match io_semaphore.acquire().await {
        Ok(p) => p,
        Err(_) => {
            return Err(CacheError::StoreUnavailable {
                store_type: StoreType::Local,
                reason: "I/O semaphore closed".to_string(),
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(100),
                },
            });
        }
    };

    // Read file
    let file_data = match fs::read(path).await {
        Ok(d) => d,
        Err(e) => {
            return Err(CacheError::Io {
                path: path.to_path_buf(),
                operation: "read cache file",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(100),
                },
            });
        }
    };

    // Deserialize header
    let header: StorageHeader = match bincode::deserialize(&file_data) {
        Ok(h) => h,
        Err(e) => {
            return Err(CacheError::Serialization {
                key: path.to_string_lossy().to_string(),
                operation: SerializationOp::Decode,
                source: Box::new(e),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }
    };

    // Validate header
    match header.validate() {
        Ok(()) => {}
        Err(e) => return Err(e),
    }

    // Calculate header size
    let header_bytes = match bincode::serialize(&header) {
        Ok(b) => b,
        Err(e) => {
            return Err(CacheError::Serialization {
                key: path.to_string_lossy().to_string(),
                operation: SerializationOp::Encode,
                source: Box::new(e),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }
    };
    let header_size = header_bytes.len();

    // Extract data portion
    if file_data.len() < header_size {
        return Err(CacheError::Corruption {
            key: path.to_string_lossy().to_string(),
            reason: "File too small after header".to_string(),
            recovery_hint: RecoveryHint::ClearAndRetry,
        });
    }
    let data = &file_data[header_size..];

    // Verify data CRC
    let actual_crc = crc32c(data);

    tracing::debug!(
        "Read validation - path: {:?}, is_compressed: {}, data_len: {}, expected_crc: {:08x}, actual_crc: {:08x}",
        path, header.is_compressed(), data.len(), header.data_crc, actual_crc
    );

    if actual_crc != header.data_crc {
        return Err(CacheError::Corruption {
            key: path.to_string_lossy().to_string(),
            reason: format!(
                "Data CRC mismatch: expected {:08x}, got {:08x}",
                header.data_crc, actual_crc
            ),
            recovery_hint: RecoveryHint::ClearAndRetry,
        });
    }

    // Decompress if needed
    if header.is_compressed() {
        match zstd_decode(data) {
            Ok(decompressed) => Ok(decompressed),
            Err(e) => Err(CacheError::Compression {
                operation: "decompress",
                source: Box::new(e),
                recovery_hint: RecoveryHint::ClearAndRetry,
            }),
        }
    } else {
        Ok(data.to_vec())
    }
}