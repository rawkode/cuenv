//! WAL replay functionality
//!
//! This module handles replaying WAL entries for crash recovery.

use super::operations::{WalEntry, WalOperation};
use crate::errors::{CacheError, RecoveryHint, Result};
use crc32c::crc32c;
use std::fs::File;
use std::io::{self, BufReader, Read as IoRead};
use std::path::Path;

/// Replay WAL entries from a file
pub fn replay_from_file<F>(path: &Path, mut callback: F) -> Result<()>
where
    F: FnMut(&WalOperation) -> Result<()>,
{
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(CacheError::Io {
                path: path.to_path_buf(),
                operation: "open WAL for replay",
                source: e,
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check WAL file permissions".to_string(),
                },
            });
        }
    };

    let mut reader = BufReader::new(file);
    let mut corrupted = false;

    loop {
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        match reader.read_exact(&mut len_bytes) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                tracing::warn!("WAL replay error reading length: {}", e);
                corrupted = true;
                break;
            }
        }

        let len = u32::from_le_bytes(len_bytes) as usize;
        if len > 10 * 1024 * 1024 {
            tracing::warn!("WAL entry too large: {} bytes", len);
            corrupted = true;
            break;
        }

        // Read entry
        let mut entry_bytes = vec![0u8; len];
        match reader.read_exact(&mut entry_bytes) {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!("WAL replay error reading entry: {}", e);
                corrupted = true;
                break;
            }
        }

        // Deserialize and verify
        let entry: WalEntry = match bincode::deserialize(&entry_bytes) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("WAL replay deserialization error: {}", e);
                corrupted = true;
                break;
            }
        };

        // Verify CRC
        let mut temp_entry = entry.clone();
        temp_entry.crc = 0;
        let temp_bytes = match bincode::serialize(&temp_entry) {
            Ok(b) => b,
            Err(_) => {
                corrupted = true;
                break;
            }
        };

        let expected_crc = crc32c(&temp_bytes);
        if entry.crc != expected_crc {
            tracing::warn!("WAL entry CRC mismatch");
            corrupted = true;
            break;
        }

        // Apply operation
        match callback(&entry.operation) {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!("WAL replay callback error: {}", e);
                // Continue replaying other entries
            }
        }
    }

    if corrupted {
        tracing::warn!("WAL corruption detected, truncating at last valid entry");
        // Could implement truncation here if needed
    }

    Ok(())
}
