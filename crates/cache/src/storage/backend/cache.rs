//! Cache-specific operations for the storage backend
//!
//! This module handles high-level cache entry operations with WAL support.

use super::StorageBackend;
use crate::errors::{CacheError, RecoveryHint, Result, SerializationOp};
use crate::storage::wal::WalOperation;
use crate::traits::CacheMetadata;
use std::path::Path;
use tokio::fs;

impl StorageBackend {
    /// Write data to cache with WAL support
    pub async fn write_cache_entry(
        &self,
        key: &str,
        metadata_path: &Path,
        data_path: &Path,
        metadata: &CacheMetadata,
        data: &[u8],
    ) -> Result<()> {
        // Serialize metadata
        let metadata_bytes = match bincode::serialize(metadata) {
            Ok(b) => b,
            Err(e) => {
                return Err(CacheError::Serialization {
                    key: key.to_string(),
                    operation: SerializationOp::Encode,
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check metadata serialization".to_string(),
                    },
                });
            }
        };

        // Create WAL operation
        let wal_op = WalOperation::Write {
            key: key.to_string(),
            metadata_path: metadata_path.to_path_buf(),
            data_path: data_path.to_path_buf(),
            metadata: metadata_bytes.clone(),
            data: data.to_vec(),
        };

        // Append to WAL first
        match self.wal.append(&wal_op) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        // Write metadata
        match self.write(metadata_path, &metadata_bytes, Some(metadata)).await {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Write data
        match self.write(data_path, data, Some(metadata)).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Try to clean up metadata
                let _ = fs::remove_file(metadata_path).await;
                Err(e)
            }
        }
    }

    /// Remove cache entry with WAL support
    pub async fn remove_cache_entry(
        &self,
        key: &str,
        metadata_path: &Path,
        data_path: &Path,
    ) -> Result<()> {
        // Create WAL operation
        let wal_op = WalOperation::Remove {
            key: key.to_string(),
            metadata_path: metadata_path.to_path_buf(),
            data_path: data_path.to_path_buf(),
        };

        // Append to WAL first
        match self.wal.append(&wal_op) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        // Remove files
        let _ = fs::remove_file(metadata_path).await;
        let _ = fs::remove_file(data_path).await;

        Ok(())
    }
}