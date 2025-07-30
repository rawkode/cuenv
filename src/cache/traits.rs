//! Core cache traits and abstractions
//!
//! This module defines the fundamental cache interface and associated types
//! for all cache implementations in the system.

use crate::cache::errors::{CacheError, Result};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::time::{Duration, SystemTime};

/// Core trait for cache operations
///
/// This trait defines the fundamental operations that all cache implementations
/// must support. It uses async methods to support both local and remote caches
/// efficiently.
#[async_trait]
pub trait Cache: Send + Sync + Debug {
    /// Get a value from the cache
    ///
    /// Returns `None` if the key doesn't exist or has expired.
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + 'static;

    /// Store a value in the cache
    ///
    /// The value will be stored with the specified TTL (time-to-live).
    /// If `ttl` is `None`, the value will be stored indefinitely.
    async fn put<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync;

    /// Remove a value from the cache
    ///
    /// Returns `Ok(true)` if the value was removed, `Ok(false)` if it didn't exist.
    async fn remove(&self, key: &str) -> Result<bool>;

    /// Check if a key exists in the cache
    async fn contains(&self, key: &str) -> Result<bool>;

    /// Get metadata about a cached entry
    async fn metadata(&self, key: &str) -> Result<Option<CacheMetadata>>;

    /// Clear all entries from the cache
    ///
    /// This operation should be used with caution in production.
    async fn clear(&self) -> Result<()>;

    /// Get cache statistics
    async fn statistics(&self) -> Result<CacheStatistics>;

    /// Perform a batch get operation
    ///
    /// Default implementation calls get() for each key sequentially.
    /// Implementations should override this for better performance.
    async fn get_many<T>(&self, keys: &[String]) -> Result<Vec<(String, Option<T>)>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            let value = self.get(key).await?;
            results.push((key.clone(), value));
        }

        Ok(results)
    }

    /// Perform a batch put operation
    ///
    /// Default implementation calls put() for each entry sequentially.
    /// Implementations should override this for better performance.
    async fn put_many<T>(&self, entries: &[(String, T, Option<Duration>)]) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        for (key, value, ttl) in entries {
            self.put(key, value, *ttl).await?;
        }

        Ok(())
    }
}

/// Metadata about a cached entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// When this entry was created
    pub created_at: SystemTime,
    /// When this entry was last accessed
    pub last_accessed: SystemTime,
    /// When this entry will expire (if applicable)
    pub expires_at: Option<SystemTime>,
    /// Size of the entry in bytes
    pub size_bytes: u64,
    /// Number of times this entry has been accessed
    pub access_count: u64,
    /// Content hash for integrity verification
    pub content_hash: String,
    /// Version of the cache format
    pub cache_version: u32,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    /// Total number of cache hits
    pub hits: u64,
    /// Total number of cache misses
    pub misses: u64,
    /// Total number of entries written
    pub writes: u64,
    /// Total number of entries removed
    pub removals: u64,
    /// Total number of errors
    pub errors: u64,
    /// Current number of entries
    pub entry_count: u64,
    /// Current total size in bytes
    pub total_bytes: u64,
    /// Maximum size in bytes (0 = unlimited)
    pub max_bytes: u64,
    /// Number of expired entries cleaned up
    pub expired_cleanups: u64,
    /// Last time statistics were reset
    pub stats_since: SystemTime,
    /// Whether compression is enabled (Phase 2)
    #[serde(default)]
    pub compression_enabled: bool,
    /// Compression ratio achieved (Phase 2)
    #[serde(default)]
    pub compression_ratio: f64,
    /// Number of WAL recoveries performed (Phase 2)
    #[serde(default)]
    pub wal_recoveries: u64,
    /// Number of checksum failures detected (Phase 2)
    #[serde(default)]
    pub checksum_failures: u64,
}

impl Default for CacheStatistics {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            writes: 0,
            removals: 0,
            errors: 0,
            entry_count: 0,
            total_bytes: 0,
            max_bytes: 0,
            expired_cleanups: 0,
            stats_since: SystemTime::now(),
            compression_enabled: false,
            compression_ratio: 0.0,
            wal_recoveries: 0,
            checksum_failures: 0,
        }
    }
}

/// Cache key validation
pub trait CacheKey: AsRef<str> {
    /// Validate that this is a valid cache key
    fn validate(&self) -> Result<()> {
        let key = self.as_ref();

        if key.is_empty() {
            return Err(CacheError::InvalidKey {
                key: key.to_string(),
                reason: "Key cannot be empty".to_string(),
                recovery_hint: crate::cache::errors::RecoveryHint::Manual {
                    instructions: "Provide a non-empty key".to_string(),
                },
            });
        }

        if key.len() > 1024 {
            return Err(CacheError::InvalidKey {
                key: format!("{}...", &key[..50]),
                reason: "Key exceeds maximum length of 1024 bytes".to_string(),
                recovery_hint: crate::cache::errors::RecoveryHint::Manual {
                    instructions: "Use a shorter key".to_string(),
                },
            });
        }

        // Check for invalid characters
        if key.contains('\0') {
            return Err(CacheError::InvalidKey {
                key: key.to_string(),
                reason: "Key contains null bytes".to_string(),
                recovery_hint: crate::cache::errors::RecoveryHint::Manual {
                    instructions: "Remove null bytes from key".to_string(),
                },
            });
        }

        Ok(())
    }
}

impl CacheKey for String {}
impl CacheKey for &str {}
impl CacheKey for std::borrow::Cow<'_, str> {}

/// Cache entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    /// The cached value
    pub value: T,
    /// Entry metadata
    pub metadata: CacheMetadata,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum cache size in bytes (0 = unlimited)
    pub max_size_bytes: u64,
    /// Default TTL for entries
    pub default_ttl: Option<Duration>,
    /// How often to run cleanup tasks
    pub cleanup_interval: Duration,
    /// Maximum number of entries (0 = unlimited)
    pub max_entries: u64,
    /// Enable compression for values above this size
    pub compression_threshold: Option<u64>,
    /// Enable encryption for sensitive data
    pub encryption_enabled: bool,
    /// Enable compression (Phase 2)
    #[serde(default = "default_compression_enabled")]
    pub compression_enabled: bool,
    /// Compression level 1-22 for zstd (Phase 2)
    #[serde(default)]
    pub compression_level: Option<i32>,
    /// Minimum size for compression in bytes (Phase 2)
    #[serde(default)]
    pub compression_min_size: Option<usize>,
}

fn default_compression_enabled() -> bool {
    true
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 1024 * 1024 * 1024, // 1GB
            default_ttl: None,
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            max_entries: 0,
            compression_threshold: Some(4096), // 4KB
            encryption_enabled: false,
            compression_enabled: true,
            compression_level: Some(3),       // Fast compression
            compression_min_size: Some(1024), // 1KB minimum
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_validation() {
        // Valid keys
        assert!("valid_key".validate().is_ok());
        assert!("path/to/resource".validate().is_ok());
        assert!("key-with-dashes".validate().is_ok());

        // Invalid keys
        assert!("".validate().is_err());
        assert!("key\0with\0nulls".validate().is_err());

        let long_key = "x".repeat(2000);
        assert!(long_key.validate().is_err());
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.max_size_bytes, 1024 * 1024 * 1024);
        assert_eq!(config.default_ttl, None);
        assert_eq!(config.cleanup_interval, Duration::from_secs(300));
    }
}
