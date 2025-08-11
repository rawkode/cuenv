//! Recovery operations for the storage backend
//!
//! This module handles WAL replay and crash recovery.

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::storage::wal::{WalOperation, WriteAheadLog};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::Semaphore;

/// Recover from WAL on startup
pub async fn recover_from_wal(wal: &Arc<WriteAheadLog>, io_semaphore: &Semaphore) -> Result<()> {
    // Collect operations first to avoid sync/async mixing
    let mut operations = Vec::new();

    wal.replay(|op| {
        operations.push(op.clone());
        Ok(())
    })?;

    // Process operations asynchronously
    for op in operations {
        let _permit = io_semaphore
            .acquire()
            .await
            .map_err(|_| CacheError::ConcurrencyConflict {
                key: "wal_recovery".to_string(),
                operation: "acquire_semaphore",
                duration: Duration::from_secs(0),
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(100),
                },
            })?;

        match op {
            WalOperation::Write {
                key: _,
                metadata_path,
                data_path,
                metadata,
                data,
            } => {
                // Just write the files directly during recovery
                let _ = fs::write(&metadata_path, &metadata).await;
                let _ = fs::write(&data_path, &data).await;
            }
            WalOperation::Remove {
                key: _,
                metadata_path,
                data_path,
            } => {
                let _ = fs::remove_file(&metadata_path).await;
                let _ = fs::remove_file(&data_path).await;
            }
            WalOperation::Clear => {
                // Clear operation would be handled at cache level
            }
            WalOperation::Checkpoint { timestamp: _ } => {
                // Nothing to do for checkpoint during recovery
            }
        }
    }

    Ok(())
}

/// Execute a single WAL operation
pub async fn execute_operation(op: &WalOperation, io: &super::StorageBackend) -> Result<()> {
    match op {
        WalOperation::Write {
            key: _,
            metadata_path,
            data_path,
            metadata,
            data,
        } => {
            // Write metadata
            match io.write(metadata_path, metadata, None).await {
                Ok(()) => {}
                Err(e) => return Err(e),
            }

            // Write data
            match io.write(data_path, data, None).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Try to clean up metadata
                    let _ = fs::remove_file(metadata_path).await;
                    Err(e)
                }
            }
        }
        WalOperation::Remove {
            key: _,
            metadata_path,
            data_path,
        } => {
            let _ = fs::remove_file(metadata_path).await;
            let _ = fs::remove_file(data_path).await;
            Ok(())
        }
        WalOperation::Clear => {
            // Clear is handled at a higher level
            Ok(())
        }
        WalOperation::Checkpoint { timestamp: _ } => {
            // Checkpoint is just a marker
            Ok(())
        }
    }
}