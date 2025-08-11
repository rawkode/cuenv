//! WAL rotation functionality
//!
//! This module handles rotating WAL files when they exceed size limits.

use super::operations::WalOperation;
use crate::errors::{CacheError, RecoveryHint, Result};
use parking_lot::Mutex;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter};
use std::path::PathBuf;
use std::time::SystemTime;

/// Rotate the WAL file
pub fn rotate_wal(
    path: &PathBuf,
    file: &Mutex<Option<BufWriter<File>>>,
    size: &Mutex<u64>,
) -> Result<()> {
    // Close current file
    *file.lock() = None;

    // Rename current WAL to backup
    let backup_path = path.with_extension(format!(
        "log.{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    ));

    match std::fs::rename(path, &backup_path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(CacheError::Io {
                path: path.clone(),
                operation: "rotate WAL",
                source: e,
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check disk space and permissions".to_string(),
                },
            });
        }
    }

    // Open new WAL
    let new_file = match OpenOptions::new().create(true).append(true).open(path) {
        Ok(f) => f,
        Err(e) => {
            return Err(CacheError::Io {
                path: path.clone(),
                operation: "open new WAL file",
                source: e,
                recovery_hint: RecoveryHint::CheckPermissions { path: path.clone() },
            });
        }
    };

    *file.lock() = Some(BufWriter::new(new_file));
    *size.lock() = 0;

    Ok(())
}

/// Write a checkpoint marker to the WAL
pub fn write_checkpoint<F>(append_fn: F) -> Result<()>
where
    F: FnOnce(&WalOperation) -> Result<u64>,
{
    let checkpoint = WalOperation::Checkpoint {
        timestamp: SystemTime::now(),
    };

    match append_fn(&checkpoint) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}