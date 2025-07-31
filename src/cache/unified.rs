//! Unified cache implementation
//!
//! This module provides a single, consolidated cache implementation that
//! replaces the previous scattered implementations (ActionCache, CacheManager,
//! ConcurrentCache, ContentAddressedStore).

use crate::cache::errors::{CacheError, RecoveryHint, Result, SerializationOp, StoreType};
use crate::cache::traits::{Cache, CacheConfig, CacheKey, CacheMetadata, CacheStatistics};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::fs;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

/// Unified cache implementation
#[derive(Clone)]
pub struct UnifiedCache {
    inner: Arc<CacheInner>,
}

struct CacheInner {
    /// Configuration
    config: CacheConfig,
    /// Base directory for file-based cache
    base_dir: PathBuf,
    /// In-memory cache for hot data
    memory_cache: DashMap<String, Arc<InMemoryEntry>>,
    /// Statistics
    stats: CacheStats,
    /// Semaphore for limiting concurrent I/O operations
    io_semaphore: Semaphore,
    /// Background cleanup task handle
    cleanup_handle: RwLock<Option<JoinHandle<()>>>,
    /// Cache format version
    version: u32,
}

struct InMemoryEntry {
    data: Vec<u8>,
    metadata: CacheMetadata,
    last_accessed: RwLock<Instant>,
}

struct CacheStats {
    hits: AtomicU64,
    misses: AtomicU64,
    writes: AtomicU64,
    removals: AtomicU64,
    errors: AtomicU64,
    total_bytes: AtomicU64,
    expired_cleanups: AtomicU64,
    stats_since: SystemTime,
}

impl UnifiedCache {
    /// Create a new unified cache
    pub async fn new(base_dir: PathBuf, config: CacheConfig) -> Result<Self> {
        // Create cache directories
        fs::create_dir_all(&base_dir)
            .await
            .map_err(|e| CacheError::Io {
                path: base_dir.clone(),
                operation: "create cache directory",
                source: e,
                recovery_hint: RecoveryHint::CheckPermissions {
                    path: base_dir.clone(),
                },
            })?;

        let objects_dir = base_dir.join("objects");
        fs::create_dir_all(&objects_dir)
            .await
            .map_err(|e| CacheError::Io {
                path: objects_dir.clone(),
                operation: "create objects directory",
                source: e,
                recovery_hint: RecoveryHint::CheckPermissions { path: objects_dir },
            })?;

        let inner = Arc::new(CacheInner {
            config,
            base_dir,
            memory_cache: DashMap::new(),
            stats: CacheStats {
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
                writes: AtomicU64::new(0),
                removals: AtomicU64::new(0),
                errors: AtomicU64::new(0),
                total_bytes: AtomicU64::new(0),
                expired_cleanups: AtomicU64::new(0),
                stats_since: SystemTime::now(),
            },
            io_semaphore: Semaphore::new(100), // Limit concurrent I/O operations
            cleanup_handle: RwLock::new(None),
            version: 1,
        });

        let cache = Self { inner };

        // Start background cleanup task
        cache.start_cleanup_task();

        Ok(cache)
    }

    /// Start the background cleanup task
    fn start_cleanup_task(&self) {
        let inner = Arc::clone(&self.inner);
        let cleanup_interval = inner.config.cleanup_interval;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                if let Err(e) = Self::cleanup_expired_entries(&inner).await {
                    tracing::warn!("Cache cleanup error: {}", e);
                }
            }
        });

        *self.inner.cleanup_handle.write() = Some(handle);
    }

    /// Clean up expired entries
    async fn cleanup_expired_entries(inner: &Arc<CacheInner>) -> Result<()> {
        let now = SystemTime::now();
        let mut expired_keys = Vec::new();

        // Find expired entries
        for entry in inner.memory_cache.iter() {
            if let Some(expires_at) = entry.value().metadata.expires_at {
                if expires_at <= now {
                    expired_keys.push(entry.key().clone());
                }
            }
        }

        // Remove expired entries
        for key in expired_keys {
            if let Some((_, entry)) = inner.memory_cache.remove(&key) {
                inner
                    .stats
                    .total_bytes
                    .fetch_sub(entry.data.len() as u64, Ordering::Relaxed);
                inner.stats.expired_cleanups.fetch_add(1, Ordering::Relaxed);
            }

            // Also remove from disk
            let path = Self::object_path(inner, &key);
            if path.exists() {
                match fs::remove_file(&path).await {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        tracing::warn!("Failed to remove expired file {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the path for a cached object
    fn object_path(inner: &CacheInner, key: &str) -> PathBuf {
        let hash = Self::hash_key(inner, key);
        let prefix = &hash[..2];
        let suffix = &hash[2..4];
        inner
            .base_dir
            .join("objects")
            .join(prefix)
            .join(suffix)
            .join(hash)
    }

    /// Hash a cache key
    fn hash_key(inner: &CacheInner, key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.update(&inner.version.to_le_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Serialize a value
    fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>> {
        bincode::serialize(value).map_err(|e| CacheError::Serialization {
            key: String::new(),
            operation: SerializationOp::Encode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check that the value is serializable".to_string(),
            },
        })
    }

    /// Deserialize a value
    fn deserialize<T: DeserializeOwned>(data: &[u8]) -> Result<T> {
        bincode::deserialize(data).map_err(|e| CacheError::Serialization {
            key: String::new(),
            operation: SerializationOp::Decode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::ClearAndRetry,
        })
    }
}

#[async_trait]
impl Cache for UnifiedCache {
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        key.validate()?;

        // Check memory cache first
        if let Some(entry) = self.inner.memory_cache.get(key) {
            // Update access time
            *entry.last_accessed.write() = Instant::now();

            // Check if expired
            if let Some(expires_at) = entry.metadata.expires_at {
                if expires_at <= SystemTime::now() {
                    // Remove expired entry
                    drop(entry);
                    self.inner.memory_cache.remove(key);
                    self.inner.stats.misses.fetch_add(1, Ordering::Relaxed);
                    return Ok(None);
                }
            }

            self.inner.stats.hits.fetch_add(1, Ordering::Relaxed);
            return Self::deserialize::<T>(&entry.data).map(Some);
        }

        // Try to load from disk
        let path = Self::object_path(&self.inner, key);

        let _permit =
            self.inner
                .io_semaphore
                .acquire()
                .await
                .map_err(|_| CacheError::StoreUnavailable {
                    store_type: StoreType::Local,
                    reason: "I/O semaphore closed".to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                })?;

        match fs::read(&path).await {
            Ok(data) => {
                // Deserialize metadata first
                let (metadata_len_bytes, rest) = data.split_at(8);
                let metadata_len =
                    u64::from_le_bytes(metadata_len_bytes.try_into().map_err(|_| {
                        CacheError::Corruption {
                            key: key.to_string(),
                            reason: "Invalid metadata length".to_string(),
                            recovery_hint: RecoveryHint::ClearAndRetry,
                        }
                    })?);

                let (metadata_bytes, value_bytes) = rest.split_at(metadata_len as usize);
                let metadata: CacheMetadata = Self::deserialize(metadata_bytes)?;

                // Check if expired
                if let Some(expires_at) = metadata.expires_at {
                    if expires_at <= SystemTime::now() {
                        // Remove expired entry
                        let _ = fs::remove_file(&path).await;
                        self.inner.stats.misses.fetch_add(1, Ordering::Relaxed);
                        return Ok(None);
                    }
                }

                // Store in memory cache for hot access
                let entry = Arc::new(InMemoryEntry {
                    data: value_bytes.to_vec(),
                    metadata: metadata.clone(),
                    last_accessed: RwLock::new(Instant::now()),
                });

                self.inner.memory_cache.insert(key.to_string(), entry);
                self.inner
                    .stats
                    .total_bytes
                    .fetch_add(value_bytes.len() as u64, Ordering::Relaxed);
                self.inner.stats.hits.fetch_add(1, Ordering::Relaxed);

                Self::deserialize::<T>(value_bytes).map(Some)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.inner.stats.misses.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            }
            Err(e) => {
                self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
                Err(CacheError::Io {
                    path,
                    operation: "read cache file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                })
            }
        }
    }

    async fn put<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        key.validate()?;

        let data = Self::serialize(value)?;
        let now = SystemTime::now();

        let metadata = CacheMetadata {
            created_at: now,
            last_accessed: now,
            expires_at: ttl.map(|d| now + d),
            size_bytes: data.len() as u64,
            access_count: 0,
            content_hash: {
                let mut hasher = Sha256::new();
                hasher.update(&data);
                format!("{:x}", hasher.finalize())
            },
            cache_version: self.inner.version,
        };

        // Check capacity
        let new_total = self
            .inner
            .stats
            .total_bytes
            .load(Ordering::Relaxed)
            .saturating_add(data.len() as u64);

        if self.inner.config.max_size_bytes > 0 && new_total > self.inner.config.max_size_bytes {
            return Err(CacheError::CapacityExceeded {
                requested_bytes: data.len() as u64,
                available_bytes: self
                    .inner
                    .config
                    .max_size_bytes
                    .saturating_sub(self.inner.stats.total_bytes.load(Ordering::Relaxed)),
                recovery_hint: RecoveryHint::IncreaseCapacity {
                    suggested_bytes: new_total,
                },
            });
        }

        // Store in memory
        let entry = Arc::new(InMemoryEntry {
            data: data.clone(),
            metadata: metadata.clone(),
            last_accessed: RwLock::new(Instant::now()),
        });

        // If we're replacing an existing entry, subtract its size
        if let Some(old_entry) = self.inner.memory_cache.insert(key.to_string(), entry) {
            self.inner
                .stats
                .total_bytes
                .fetch_sub(old_entry.data.len() as u64, Ordering::Relaxed);
        }

        self.inner
            .stats
            .total_bytes
            .fetch_add(data.len() as u64, Ordering::Relaxed);

        // Write to disk
        let path = Self::object_path(&self.inner, key);
        let parent = path.parent().ok_or_else(|| CacheError::Configuration {
            message: "Invalid cache path".to_string(),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check cache configuration".to_string(),
            },
        })?;

        let _permit =
            self.inner
                .io_semaphore
                .acquire()
                .await
                .map_err(|_| CacheError::StoreUnavailable {
                    store_type: StoreType::Local,
                    reason: "I/O semaphore closed".to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                })?;

        fs::create_dir_all(parent)
            .await
            .map_err(|e| CacheError::Io {
                path: parent.to_path_buf(),
                operation: "create cache directory",
                source: e,
                recovery_hint: RecoveryHint::CheckPermissions {
                    path: parent.to_path_buf(),
                },
            })?;

        // Serialize metadata and value together
        let metadata_bytes = Self::serialize(&metadata)?;
        let mut combined = Vec::with_capacity(8 + metadata_bytes.len() + data.len());
        combined.extend_from_slice(&(metadata_bytes.len() as u64).to_le_bytes());
        combined.extend_from_slice(&metadata_bytes);
        combined.extend_from_slice(&data);

        // Write atomically with unique temp file name
        let temp_path = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        fs::write(&temp_path, &combined)
            .await
            .map_err(|e| CacheError::Io {
                path: temp_path.clone(),
                operation: "write cache file",
                source: e,
                recovery_hint: RecoveryHint::CheckPermissions {
                    path: temp_path.clone(),
                },
            })?;

        fs::rename(&temp_path, &path)
            .await
            .map_err(|e| CacheError::Io {
                path: path.clone(),
                operation: "rename cache file",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(10),
                },
            })?;

        self.inner.stats.writes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<bool> {
        key.validate()?;

        let mut removed = false;

        // Remove from memory
        if let Some((_, entry)) = self.inner.memory_cache.remove(key) {
            self.inner
                .stats
                .total_bytes
                .fetch_sub(entry.data.len() as u64, Ordering::Relaxed);
            removed = true;
        }

        // Remove from disk
        let path = Self::object_path(&self.inner, key);
        match fs::remove_file(&path).await {
            Ok(()) => {
                removed = true;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path,
                    operation: "remove cache file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        }

        if removed {
            self.inner.stats.removals.fetch_add(1, Ordering::Relaxed);
        }

        Ok(removed)
    }

    async fn contains(&self, key: &str) -> Result<bool> {
        key.validate()?;

        // Check memory first
        if let Some(entry) = self.inner.memory_cache.get(key) {
            // Check if expired
            if let Some(expires_at) = entry.metadata.expires_at {
                if expires_at <= SystemTime::now() {
                    return Ok(false);
                }
            }
            return Ok(true);
        }

        // Check disk
        let path = Self::object_path(&self.inner, key);
        if !path.exists() {
            return Ok(false);
        }

        // Need to check if the disk entry is expired
        // We'll read just the metadata to check expiration
        let _permit =
            self.inner
                .io_semaphore
                .acquire()
                .await
                .map_err(|_| CacheError::StoreUnavailable {
                    store_type: StoreType::Local,
                    reason: "I/O semaphore closed".to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                })?;

        match fs::read(&path).await {
            Ok(data) => {
                // Deserialize metadata to check expiration
                if data.len() < 8 {
                    return Ok(false);
                }

                let (metadata_len_bytes, rest) = data.split_at(8);
                let Ok(metadata_len_array) = metadata_len_bytes.try_into() else {
                    return Ok(false);
                };
                let metadata_len = u64::from_le_bytes(metadata_len_array);

                if rest.len() < metadata_len as usize {
                    return Ok(false);
                }

                let metadata_bytes = &rest[..metadata_len as usize];
                let metadata: CacheMetadata = Self::deserialize(metadata_bytes)?;

                // Check if expired
                if let Some(expires_at) = metadata.expires_at {
                    if expires_at <= SystemTime::now() {
                        // It's expired, so it doesn't exist
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    async fn metadata(&self, key: &str) -> Result<Option<CacheMetadata>> {
        key.validate()?;

        // Check memory first
        if let Some(entry) = self.inner.memory_cache.get(key) {
            return Ok(Some(entry.metadata.clone()));
        }

        // Try to load from disk (just metadata)
        let path = Self::object_path(&self.inner, key);
        match fs::read(&path).await {
            Ok(data) => {
                if data.len() < 8 {
                    return Ok(None);
                }

                let (metadata_len_bytes, rest) = data.split_at(8);
                let metadata_len = match metadata_len_bytes.try_into() {
                    Ok(bytes) => u64::from_le_bytes(bytes),
                    Err(_) => {
                        return Err(CacheError::Corruption {
                            key: key.to_string(),
                            reason: "Invalid metadata length".to_string(),
                            recovery_hint: RecoveryHint::ClearAndRetry,
                        });
                    }
                };

                let metadata_bytes = &rest[..metadata_len as usize];
                let metadata: CacheMetadata = Self::deserialize(metadata_bytes)?;

                Ok(Some(metadata))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(CacheError::Io {
                path,
                operation: "read cache metadata",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(100),
                },
            }),
        }
    }

    async fn clear(&self) -> Result<()> {
        // Clear memory cache
        self.inner.memory_cache.clear();
        self.inner.stats.total_bytes.store(0, Ordering::Relaxed);

        // Clear disk cache
        let objects_dir = self.inner.base_dir.join("objects");
        if objects_dir.exists() {
            match fs::remove_dir_all(&objects_dir).await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: objects_dir.clone(),
                        operation: "clear cache directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions {
                            path: objects_dir.clone(),
                        },
                    });
                }
            }

            match fs::create_dir_all(&objects_dir).await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: objects_dir.clone(),
                        operation: "recreate cache directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions { path: objects_dir },
                    });
                }
            }

            // Also clear metadata directory
            let metadata_dir = self.inner.base_dir.join("metadata");
            if metadata_dir.exists() {
                match fs::remove_dir_all(&metadata_dir).await {
                    Ok(()) => {}
                    Err(e) => {
                        return Err(CacheError::Io {
                            path: metadata_dir.clone(),
                            operation: "clear metadata directory",
                            source: e,
                            recovery_hint: RecoveryHint::CheckPermissions {
                                path: metadata_dir.clone(),
                            },
                        });
                    }
                }

                match fs::create_dir_all(&metadata_dir).await {
                    Ok(()) => {}
                    Err(e) => {
                        return Err(CacheError::Io {
                            path: metadata_dir.clone(),
                            operation: "recreate metadata directory",
                            source: e,
                            recovery_hint: RecoveryHint::CheckPermissions { path: metadata_dir },
                        });
                    }
                }
            }
        }

        Ok(())
    }

    async fn statistics(&self) -> Result<CacheStatistics> {
        let entry_count = self.inner.memory_cache.len() as u64;

        Ok(CacheStatistics {
            hits: self.inner.stats.hits.load(Ordering::Relaxed),
            misses: self.inner.stats.misses.load(Ordering::Relaxed),
            writes: self.inner.stats.writes.load(Ordering::Relaxed),
            removals: self.inner.stats.removals.load(Ordering::Relaxed),
            errors: self.inner.stats.errors.load(Ordering::Relaxed),
            entry_count,
            total_bytes: self.inner.stats.total_bytes.load(Ordering::Relaxed),
            max_bytes: self.inner.config.max_size_bytes,
            expired_cleanups: self.inner.stats.expired_cleanups.load(Ordering::Relaxed),
            stats_since: self.inner.stats.stats_since,
            compression_enabled: self.inner.config.compression_enabled,
            compression_ratio: 1.0, // TODO: Track actual compression ratio
            wal_recoveries: 0,      // TODO: Track WAL recoveries
            checksum_failures: 0,   // TODO: Track checksum failures
        })
    }
}

impl fmt::Debug for UnifiedCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnifiedCache")
            .field("base_dir", &self.inner.base_dir)
            .field("version", &self.inner.version)
            .field("entry_count", &self.inner.memory_cache.len())
            .finish()
    }
}

impl Drop for CacheInner {
    fn drop(&mut self) {
        // Cancel cleanup task
        if let Some(handle) = self.cleanup_handle.write().take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_basic_operations() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let cache =
            UnifiedCache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

        // Test put and get
        cache.put("key1", &"value1", None).await?;
        let value: Option<String> = cache.get("key1").await?;
        assert_eq!(value, Some("value1".to_string()));

        // Test contains
        assert!(cache.contains("key1").await?);
        assert!(!cache.contains("key2").await?);

        // Test remove
        assert!(cache.remove("key1").await?);
        assert!(!cache.contains("key1").await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_expiration() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let cache =
            UnifiedCache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

        // Put with short TTL
        cache
            .put("expires", &"soon", Some(Duration::from_millis(50)))
            .await?;

        // Should exist immediately
        assert!(cache.contains("expires").await?);

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be expired
        let value: Option<String> = cache.get("expires").await?;
        assert_eq!(value, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_statistics() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let cache =
            UnifiedCache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

        cache.put("key1", &"value1", None).await?;
        let _: Option<String> = cache.get("key1").await?; // Hit
        let _: Option<String> = cache.get("key2").await?; // Miss
        cache.remove("key1").await?;

        let stats = cache.statistics().await?;
        assert_eq!(stats.writes, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.removals, 1);

        Ok(())
    }
}
