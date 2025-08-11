//! Path generation and hashing utilities for the cache

use sha2::{Digest, Sha256};
use std::path::PathBuf;

use super::types::CacheInner;

/// Generate object path using optimized 256-shard distribution
#[inline(always)]
pub fn object_path(inner: &CacheInner, key: &str) -> PathBuf {
    let hash = hash_key(inner, key);
    // Use first byte of hash for 256-way sharding (00-ff)
    // This provides optimal distribution for file systems
    let shard = &hash[..2];
    inner.base_dir.join("objects").join(shard).join(&hash)
}

/// Get object path from hash (for cleanup)
pub fn object_path_from_hash(inner: &CacheInner, hash: &str) -> PathBuf {
    let shard = &hash[..2];
    inner.base_dir.join("objects").join(shard).join(hash)
}

/// Generate metadata path using optimized 256-shard distribution
#[inline(always)]
pub fn metadata_path(inner: &CacheInner, key: &str) -> PathBuf {
    let hash = hash_key(inner, key);
    // Use first byte of hash for 256-way sharding (00-ff)
    let shard = &hash[..2];
    inner
        .base_dir
        .join("metadata")
        .join(shard)
        .join(format!("{}.meta", &hash))
}

/// Hash a cache key with performance optimizations
#[inline(always)]
pub fn hash_key(inner: &CacheInner, key: &str) -> String {
    // Use SIMD-accelerated hashing when available
    #[cfg(target_arch = "x86_64")]
    {
        use crate::performance::simd_hash;
        if simd_hash::is_simd_available() {
            let simd_hash = unsafe { simd_hash::hash_key_simd(key.as_bytes()) };
            // Mix with version for cache invalidation
            let mixed = simd_hash ^ (inner.version as u64);
            return format!("{mixed:016x}");
        }
    }

    // Fallback to SHA256 for cryptographic strength
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher.update(inner.version.to_le_bytes());
    format!("{:x}", hasher.finalize())
}
