//! Hash computation functions for the Merkle tree

use crate::security::merkle::nodes::{CacheEntryMetadata, Hash};
use sha2::{Digest, Sha256};

/// Compute hash for a leaf node
pub fn compute_leaf_hash(
    cache_key: &str,
    content_hash: &Hash,
    metadata: &CacheEntryMetadata,
) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(b"LEAF:");
    hasher.update(cache_key.as_bytes());
    hasher.update(content_hash);
    hasher.update(metadata.size_bytes.to_le_bytes());
    hasher.update(metadata.modified_at.to_le_bytes());
    if let Some(expires_at) = metadata.expires_at {
        hasher.update(expires_at.to_le_bytes());
    }
    hasher.finalize().into()
}

/// Compute hash for an internal node
pub fn compute_internal_hash(left_hash: Hash, right_hash: Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(b"INTERNAL:");
    hasher.update(left_hash);
    hasher.update(right_hash);
    hasher.finalize().into()
}
