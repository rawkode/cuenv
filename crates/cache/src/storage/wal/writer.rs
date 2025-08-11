//! WAL writer implementation
//!
//! This module provides the main write-ahead log functionality.

use super::append::append_operation;
use super::operations::WalOperation;
use super::replay::replay_from_file;
use super::rotation::{rotate_wal, write_checkpoint};
use crate::errors::{CacheError, RecoveryHint, Result};
use parking_lot::Mutex;
use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Write-Ahead Log for atomic operations
pub struct WriteAheadLog {
    /// Path to the WAL file
    path: PathBuf,
    /// Current WAL file handle
    file: Mutex<Option<BufWriter<File>>>,
    /// Current size of the WAL
    size: Arc<Mutex<u64>>,
    /// Sequence number for operations
    sequence: Arc<Mutex<u64>>,
}

impl WriteAheadLog {
    pub fn new(base_dir: &Path) -> Result<Self> {
        let wal_dir = base_dir.join("wal");
        match std::fs::create_dir_all(&wal_dir) {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: wal_dir.clone(),
                    operation: "create WAL directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: wal_dir },
                });
            }
        }

        let path = wal_dir.join("wal.log");
        let wal = Self {
            path: path.clone(),
            file: Mutex::new(None),
            size: Arc::new(Mutex::new(0)),
            sequence: Arc::new(Mutex::new(0)),
        };

        // Open or create the WAL file
        match wal.open_or_create() {
            Ok(()) => Ok(wal),
            Err(e) => Err(e),
        }
    }

    fn open_or_create(&self) -> Result<()> {
        let file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(f) => f,
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "open WAL file",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: self.path.clone(),
                    },
                });
            }
        };

        // Get current file size
        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "get WAL metadata",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        };

        *self.size.lock() = metadata.len();
        *self.file.lock() = Some(BufWriter::new(file));

        Ok(())
    }

    pub fn append(&self, op: &WalOperation) -> Result<u64> {
        let (seq, needs_rotation) =
            append_operation(op, &self.path, &self.file, &self.size, &self.sequence)?;

        if needs_rotation {
            match self.rotate() {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }

        Ok(seq)
    }

    fn rotate(&self) -> Result<()> {
        // Perform rotation
        rotate_wal(&self.path, &self.file, &self.size)?;

        // Re-open the file
        self.open_or_create()?;

        // Write checkpoint to new WAL
        write_checkpoint(|op| self.append(op))
    }

    pub fn replay<F>(&self, callback: F) -> Result<()>
    where
        F: FnMut(&WalOperation) -> Result<()>,
    {
        replay_from_file(&self.path, callback)
    }
}
