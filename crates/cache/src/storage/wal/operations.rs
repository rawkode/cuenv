//! WAL operation types and entries
//!
//! This module defines the operations that can be recorded
//! in the Write-Ahead Log for crash recovery.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// Write-Ahead Log entry type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOperation {
    /// Write a new cache entry
    Write {
        key: String,
        metadata_path: PathBuf,
        data_path: PathBuf,
        metadata: Vec<u8>,
        data: Vec<u8>,
    },
    /// Remove a cache entry
    Remove {
        key: String,
        metadata_path: PathBuf,
        data_path: PathBuf,
    },
    /// Clear all cache entries
    Clear,
    /// Checkpoint - all operations before this are committed
    Checkpoint { timestamp: SystemTime },
}

/// WAL entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub sequence: u64,
    pub timestamp: SystemTime,
    pub operation: WalOperation,
    pub crc: u32,
}