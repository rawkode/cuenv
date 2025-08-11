//! File finalization utilities for cache writers
//!
//! Handles the complex atomics operations required for safely
//! finalizing cache writes with metadata management.

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::CacheMetadata;
use std::path::PathBuf;
use std::time::Duration;

/// Helper to atomically finalize cache files
pub async fn finalize_cache_files(
    temp_path: &PathBuf,
    final_path: &PathBuf,
    metadata_path: &PathBuf,
    metadata: &CacheMetadata,
) -> Result<()> {
    // Write metadata
    let metadata_bytes = match bincode::serialize(metadata) {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err(CacheError::Serialization {
                key: String::new(),
                operation: crate::errors::SerializationOp::Encode,
                source: Box::new(e),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check metadata serialization".to_string(),
                },
            });
        }
    };

    // Ensure metadata directory exists
    if let Some(parent) = metadata_path.parent() {
        match tokio::fs::create_dir_all(parent).await {
            Ok(()) => {}
            Err(e) => {
                let _ = tokio::fs::remove_file(temp_path).await;
                return Err(CacheError::Io {
                    path: parent.to_path_buf(),
                    operation: "create metadata directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: parent.to_path_buf(),
                    },
                });
            }
        }
    }

    let temp_metadata = metadata_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
    match tokio::fs::write(&temp_metadata, &metadata_bytes).await {
        Ok(()) => {}
        Err(e) => {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err(CacheError::Io {
                path: temp_metadata.clone(),
                operation: "write cache metadata",
                source: e,
                recovery_hint: RecoveryHint::CheckPermissions {
                    path: temp_metadata,
                },
            });
        }
    }

    // Atomic rename of both files
    match tokio::fs::rename(&temp_metadata, metadata_path).await {
        Ok(()) => {}
        Err(e) => {
            let _ = tokio::fs::remove_file(temp_path).await;
            let _ = tokio::fs::remove_file(&temp_metadata).await;
            return Err(CacheError::Io {
                path: metadata_path.clone(),
                operation: "rename metadata file",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            });
        }
    }

    match tokio::fs::rename(temp_path, final_path).await {
        Ok(()) => {}
        Err(e) => {
            // Try to clean up metadata since data rename failed
            let _ = tokio::fs::remove_file(metadata_path).await;
            return Err(CacheError::Io {
                path: final_path.clone(),
                operation: "rename cache data file",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            });
        }
    }

    Ok(())
}
