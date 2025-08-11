//! Shared utilities for cache operations

use crate::errors::{CacheError, RecoveryHint, Result, SerializationOp};
use memmap2::{Mmap, MmapOptions};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

/// Serialize a value
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    match bincode::serialize(value) {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(CacheError::Serialization {
            key: String::new(),
            operation: SerializationOp::Encode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check that the value is serializable".to_string(),
            },
        }),
    }
}

/// Deserialize a value
pub fn deserialize<T: DeserializeOwned>(data: &[u8]) -> Result<T> {
    match bincode::deserialize(data) {
        Ok(value) => Ok(value),
        Err(e) => Err(CacheError::Serialization {
            key: String::new(),
            operation: SerializationOp::Decode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::ClearAndRetry,
        }),
    }
}

/// Memory-map a file for zero-copy access
pub fn mmap_file(path: &PathBuf) -> Result<Mmap> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            return Err(CacheError::Io {
                path: path.clone(),
                operation: "open file for mmap",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            });
        }
    };

    match unsafe { MmapOptions::new().map(&file) } {
        Ok(mmap) => Ok(mmap),
        Err(e) => Err(CacheError::Io {
            path: path.clone(),
            operation: "memory-map file",
            source: e,
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check available memory and file permissions".to_string(),
            },
        }),
    }
}
