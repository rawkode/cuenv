//! Path utilities for cache storage
//!
//! Provides utilities for computing hash-based paths and managing
//! cache storage organization with sharding.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Hash a cache key with version information
pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher.update(3u32.to_le_bytes()); // Version 3
    format!("{:x}", hasher.finalize())
}

/// Get the storage paths for a given hash
pub fn get_paths(cache_dir: &Path, hash: &str) -> (PathBuf, PathBuf) {
    // Use 256-way sharding as specified in Phase 3
    let shard = &hash[..2];

    let data_path = cache_dir.join("objects").join(shard).join(hash);

    let metadata_path = cache_dir
        .join("metadata")
        .join(shard)
        .join(format!("{hash}.meta"));

    (data_path, metadata_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_hashing() {
        let key = "test_key";
        let hash1 = hash_key(key);
        let hash2 = hash_key(key);

        // Same key should produce same hash
        assert_eq!(hash1, hash2);

        // Different keys should produce different hashes
        let different_hash = hash_key("different_key");
        assert_ne!(hash1, different_hash);

        // Hash should be deterministic with version
        assert_eq!(hash1.len(), 64); // SHA256 hex string length
    }

    #[test]
    fn test_path_generation() {
        let cache_dir = std::path::Path::new("/tmp/cache");
        let hash = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";

        let (data_path, metadata_path) = get_paths(cache_dir, hash);

        assert_eq!(data_path, cache_dir.join("objects").join("ab").join(hash));

        assert_eq!(
            metadata_path,
            cache_dir
                .join("metadata")
                .join("ab")
                .join(format!("{hash}.meta"))
        );
    }
}
