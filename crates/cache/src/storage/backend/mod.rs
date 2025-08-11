//! Storage backend implementation
//!
//! This module provides the core storage functionality with
//! compression, checksums, and atomic operations.

mod cache;
mod reader;
mod recovery;
mod writer;

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::storage::compression::{CompressionConfig, CompressionStats};
use crate::storage::transaction::TransactionManager;
use crate::storage::wal::{WalOperation, WriteAheadLog};
use crate::traits::CacheMetadata;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Semaphore;

/// Storage backend for the cache system
pub struct StorageBackend {
    /// Base directory for storage
    #[allow(dead_code)]
    base_dir: PathBuf,
    /// Write-ahead log
    wal: Arc<WriteAheadLog>,
    /// Compression configuration
    compression: CompressionConfig,
    /// I/O semaphore for rate limiting
    io_semaphore: Arc<Semaphore>,
    /// Transaction manager
    transactions: TransactionManager,
}

impl StorageBackend {
    /// Create a new storage backend
    pub async fn new(base_dir: PathBuf, compression: CompressionConfig) -> Result<Self> {
        // Create base directory
        match fs::create_dir_all(&base_dir).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: base_dir.clone(),
                    operation: "create storage directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: base_dir },
                });
            }
        }

        // Initialize WAL
        let wal = match WriteAheadLog::new(&base_dir) {
            Ok(w) => Arc::new(w),
            Err(e) => return Err(e),
        };

        let backend = Self {
            base_dir,
            wal,
            compression,
            io_semaphore: Arc::new(Semaphore::new(100)),
            transactions: TransactionManager::new(),
        };

        // Replay WAL to recover from any crashes
        match recovery::recover_from_wal(&backend.wal, &backend.io_semaphore).await {
            Ok(()) => Ok(backend),
            Err(e) => Err(e),
        }
    }

    /// Begin a new transaction
    pub fn begin_transaction(&self) -> u64 {
        self.transactions.begin()
    }

    /// Add an operation to a transaction
    pub fn add_to_transaction(&self, tx_id: u64, op: WalOperation) -> Result<()> {
        self.transactions.add_operation(tx_id, op)
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self, tx_id: u64) -> Result<()> {
        let ops = self.transactions.take_operations(tx_id)?;

        // Write all operations to WAL first
        for op in &ops {
            match self.wal.append(op) {
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }

        // Execute all operations
        for op in ops {
            match recovery::execute_operation(&op, self).await {
                Ok(()) => {}
                Err(e) => {
                    // Log error but continue - WAL has the operation for retry
                    tracing::error!("Failed to execute operation: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback_transaction(&self, tx_id: u64) {
        self.transactions.rollback(tx_id);
    }

    /// Write data with compression and checksums
    pub async fn write(
        &self,
        path: &Path,
        data: &[u8],
        metadata: Option<&CacheMetadata>,
    ) -> Result<()> {
        writer::write_data(path, data, metadata, &self.compression, &self.io_semaphore).await
    }

    /// Read data with decompression and checksum verification
    pub async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        reader::read_data(path, &self.io_semaphore).await
    }

    /// Get compression statistics
    pub fn compression_stats(&self) -> CompressionStats {
        CompressionStats {
            enabled: self.compression.enabled,
            level: self.compression.level,
            min_size: self.compression.min_size,
        }
    }
}
