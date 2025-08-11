//! WAL append operations
//!
//! This module handles appending entries to the write-ahead log.

use super::operations::{WalEntry, WalOperation};
use crate::errors::{CacheError, RecoveryHint, Result, SerializationOp, StoreType};
use crate::storage::format::MAX_WAL_SIZE;
use crc32c::crc32c;
use parking_lot::Mutex;
use std::fs::File;
use std::io::{BufWriter, Write as IoWrite};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Append an operation to the WAL
pub fn append_operation(
    op: &WalOperation,
    path: &PathBuf,
    file: &Mutex<Option<BufWriter<File>>>,
    size: &Arc<Mutex<u64>>,
    sequence: &Arc<Mutex<u64>>,
) -> Result<(u64, bool)> {
    let mut file_guard = file.lock();
    let file_writer = match file_guard.as_mut() {
        Some(f) => f,
        None => {
            return Err(CacheError::StoreUnavailable {
                store_type: StoreType::Local,
                reason: "WAL not initialized".to_string(),
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            });
        }
    };

    // Get next sequence number
    let seq = {
        let mut seq_guard = sequence.lock();
        *seq_guard += 1;
        *seq_guard
    };

    // Create WAL entry
    let entry = WalEntry {
        sequence: seq,
        timestamp: SystemTime::now(),
        operation: op.clone(),
        crc: 0,
    };

    // Serialize entry
    let mut entry_bytes = match bincode::serialize(&entry) {
        Ok(b) => b,
        Err(e) => {
            return Err(CacheError::Serialization {
                key: String::new(),
                operation: SerializationOp::Encode,
                source: Box::new(e),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check WAL entry serialization".to_string(),
                },
            });
        }
    };

    // Calculate and set CRC
    let crc = crc32c(&entry_bytes);
    if let Ok(mut entry_with_crc) = bincode::deserialize::<WalEntry>(&entry_bytes) {
        entry_with_crc.crc = crc;
        entry_bytes = match bincode::serialize(&entry_with_crc) {
            Ok(b) => b,
            Err(e) => {
                return Err(CacheError::Serialization {
                    key: String::new(),
                    operation: SerializationOp::Encode,
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check WAL entry serialization".to_string(),
                    },
                });
            }
        };
    }

    // Write length prefix + entry
    let len_bytes = (entry_bytes.len() as u32).to_le_bytes();
    match file_writer.write_all(&len_bytes) {
        Ok(()) => {}
        Err(e) => {
            return Err(CacheError::Io {
                path: path.clone(),
                operation: "write WAL length",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            });
        }
    }

    match file_writer.write_all(&entry_bytes) {
        Ok(()) => {}
        Err(e) => {
            return Err(CacheError::Io {
                path: path.clone(),
                operation: "write WAL entry",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            });
        }
    }

    // Sync to disk for durability
    match file_writer.flush() {
        Ok(()) => {}
        Err(e) => {
            return Err(CacheError::Io {
                path: path.clone(),
                operation: "flush WAL",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            });
        }
    }

    // Update size
    let entry_size = 4 + entry_bytes.len() as u64;
    let mut size_guard = size.lock();
    *size_guard += entry_size;

    // Check if we need to rotate
    let needs_rotation = *size_guard > MAX_WAL_SIZE;

    Ok((seq, needs_rotation))
}
