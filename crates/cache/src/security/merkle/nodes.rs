//! Node types and hash utilities for the Merkle tree

use crate::errors::{CacheError, RecoveryHint, Result};
use serde::{Deserialize, Serialize};

/// Cryptographic hash type (SHA-256)
pub type Hash = [u8; 32];

/// Convert bytes to hex string for display
pub fn hash_to_hex(hash: &Hash) -> String {
    hex::encode(hash)
}

/// Convert hex string to hash
#[allow(dead_code)]
pub fn hex_to_hash(hex: &str) -> Result<Hash> {
    if hex.len() != 64 {
        return Err(CacheError::Configuration {
            message: format!("Invalid hash length: expected 64 chars, got {}", hex.len()),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Provide valid SHA-256 hash".to_string(),
            },
        });
    }

    let mut hash = [0u8; 32];
    match hex::decode_to_slice(hex, &mut hash) {
        Ok(()) => Ok(hash),
        Err(e) => Err(CacheError::Configuration {
            message: format!("Invalid hex hash: {e}"),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Provide valid hexadecimal hash".to_string(),
            },
        }),
    }
}

/// Merkle tree node containing hash and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    /// Node hash value
    pub hash: Hash,
    /// Left child hash (if internal node)
    pub left_child: Option<Hash>,
    /// Right child hash (if internal node)
    pub right_child: Option<Hash>,
    /// Node depth in tree (0 = leaf)
    pub depth: u32,
    /// Cache key (if leaf node)
    pub cache_key: Option<String>,
    /// Cache entry metadata (if leaf node)
    pub entry_metadata: Option<CacheEntryMetadata>,
}

/// Metadata for cache entries in Merkle tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntryMetadata {
    /// Entry size in bytes
    pub size_bytes: u64,
    /// Last modification timestamp
    pub modified_at: u64,
    /// Content hash of the cached data
    pub content_hash: Hash,
    /// Entry TTL expiration (if applicable)
    pub expires_at: Option<u64>,
}
