//! Write operations for the storage backend
//!
//! This module handles writing data with compression
//! and checksum generation.

use crate::errors::{CacheError, RecoveryHint, Result, SerializationOp, StoreType};
use crate::storage::compression::CompressionConfig;
use crate::storage::format::StorageHeader;
use crate::traits::CacheMetadata;
use crc32c::crc32c;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::sync::Semaphore;
use zstd::stream::encode_all as zstd_encode;

/// Write data with compression and checksums
pub async fn write_data(
    path: &Path,
    data: &[u8],
    _metadata: Option<&CacheMetadata>,
    compression: &CompressionConfig,
    io_semaphore: &Semaphore,
) -> Result<()> {
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

    // Decide whether to compress
    let should_compress = compression.enabled && data.len() >= compression.min_size;

    tracing::debug!(
        "Write decision - path: {:?}, data_len: {}, min_size: {}, should_compress: {}",
        path,
        data.len(),
        compression.min_size,
        should_compress
    );

    // Compress if needed
    let (compressed_data, compressed_size, uncompressed_size) = if should_compress {
        match zstd_encode(data, compression.level) {
            Ok(compressed) => {
                let compressed_len = compressed.len();
                tracing::debug!(
                    "Compressed data - original: {}, compressed: {}",
                    data.len(),
                    compressed_len
                );
                (compressed, compressed_len as u64, data.len() as u64)
            }
            Err(e) => {
                return Err(CacheError::Compression {
                    operation: "compress",
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check compression settings".to_string(),
                    },
                });
            }
        }
    } else {
        (data.to_vec(), data.len() as u64, data.len() as u64)
    };

    // Calculate data CRC
    let data_crc = crc32c(&compressed_data);

    // Create header
    let header = StorageHeader::new(
        uncompressed_size,
        compressed_size,
        data_crc,
        should_compress,
    );

    // Serialize header
    let header_bytes = match bincode::serialize(&header) {
        Ok(b) => b,
        Err(e) => {
            return Err(CacheError::Serialization {
                key: path.to_string_lossy().to_string(),
                operation: SerializationOp::Encode,
                source: Box::new(e),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check header serialization".to_string(),
                },
            });
        }
    };

    // Write atomically
    let temp_path = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));

    // Ensure parent directory exists
    if let Some(parent) = temp_path.parent() {
        match fs::create_dir_all(parent).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: parent.to_path_buf(),
                    operation: "create parent directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: parent.to_path_buf(),
                    },
                });
            }
        }
    }

    // Write header + data
    let mut output = Vec::with_capacity(header_bytes.len() + compressed_data.len());
    output.extend_from_slice(&header_bytes);
    output.extend_from_slice(&compressed_data);

    match fs::write(&temp_path, &output).await {
        Ok(()) => {}
        Err(e) => {
            return Err(CacheError::Io {
                path: temp_path.clone(),
                operation: "write cache file",
                source: e,
                recovery_hint: RecoveryHint::CheckPermissions { path: temp_path },
            });
        }
    }

    // Atomic rename
    match fs::rename(&temp_path, path).await {
        Ok(()) => Ok(()),
        Err(e) => {
            // Clean up temp file
            let _ = fs::remove_file(&temp_path).await;
            Err(CacheError::Io {
                path: path.to_path_buf(),
                operation: "atomic rename",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            })
        }
    }
}
