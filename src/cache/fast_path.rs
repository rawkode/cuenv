//! Fast path optimizations for common cache operations
//!
//! This module provides specialized implementations for hot code paths
//! to minimize latency and maximize throughput.

use crate::cache::errors::Result;
use crate::cache::traits::{CacheKey, CacheMetadata};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Fast path cache for small, frequently accessed values
pub struct FastPathCache {
    /// Small value cache (up to 1KB)
    small_values: DashMap<String, Arc<SmallValue>>,
    /// Maximum size for small values
    small_value_threshold: usize,
    /// LRU tracking for eviction
    lru_tracker: Arc<RwLock<LruTracker>>,
}

struct SmallValue {
    data: Vec<u8>,
    metadata: CacheMetadata,
    last_access: RwLock<Instant>,
}

struct LruTracker {
    /// Access order tracking
    access_order: Vec<String>,
    /// Maximum entries to track
    max_entries: usize,
}

impl FastPathCache {
    pub const fn new(small_value_threshold: usize, max_entries: usize) -> Self {
        Self {
            small_values: DashMap::new(),
            small_value_threshold,
            lru_tracker: Arc::new(RwLock::new(LruTracker {
                access_order: Vec::new(),
                max_entries,
            })),
        }
    }

    /// Fast path get for small values
    #[inline(always)]
    pub fn get_small(&self, key: &str) -> Option<(Vec<u8>, CacheMetadata)> {
        if let Some(entry) = self.small_values.get(key) {
            // Check expiration without allocation
            if let Some(expires_at) = entry.metadata.expires_at {
                if expires_at <= SystemTime::now() {
                    // Expired - remove and return None
                    drop(entry);
                    self.small_values.remove(key);
                    return None;
                }
            }

            // Update access time
            *entry.last_access.write() = Instant::now();

            // Clone data for return (small, so fast)
            Some((entry.data.clone(), entry.metadata.clone()))
        } else {
            None
        }
    }

    /// Fast path put for small values
    #[inline(always)]
    pub fn put_small(&self, key: String, data: Vec<u8>, metadata: CacheMetadata) -> bool {
        if data.len() > self.small_value_threshold {
            return false; // Too large for fast path
        }

        let entry = Arc::new(SmallValue {
            data,
            metadata,
            last_access: RwLock::new(Instant::now()),
        });

        // Check if we need to evict
        if self.small_values.len() >= self.lru_tracker.read().max_entries {
            self.evict_lru();
        }

        self.small_values.insert(key.clone(), entry);

        // Track in LRU
        if let Ok(mut tracker) = self.lru_tracker.try_write() {
            tracker.access_order.retain(|k| k != &key);
            tracker.access_order.push(key);
        }

        true
    }

    /// Evict least recently used entry
    fn evict_lru(&self) {
        if let Ok(mut tracker) = self.lru_tracker.try_write() {
            if let Some(key) = tracker.access_order.first() {
                let key = key.clone();
                tracker.access_order.remove(0);
                self.small_values.remove(&key);
            }
        }
    }

    /// Clear all fast path entries
    pub fn clear(&self) {
        self.small_values.clear();
        if let Ok(mut tracker) = self.lru_tracker.try_write() {
            tracker.access_order.clear();
        }
    }
}

/// Specialized implementations for common value types
pub mod specialized {
    use super::*;
    use serde::{Deserialize, Serialize};

    /// Fast path for string values
    #[inline(always)]
    pub fn get_string(cache: &FastPathCache, key: &str) -> Option<String> {
        cache
            .get_small(key)
            .and_then(|(data, _)| String::from_utf8(data).ok())
    }

    /// Fast path for boolean flags
    #[inline(always)]
    pub fn get_bool(cache: &FastPathCache, key: &str) -> Option<bool> {
        cache.get_small(key).and_then(|(data, _)| {
            if data.len() == 1 {
                Some(data[0] != 0)
            } else {
                None
            }
        })
    }

    /// Fast path for u64 values
    #[inline(always)]
    pub fn get_u64(cache: &FastPathCache, key: &str) -> Option<u64> {
        cache.get_small(key).and_then(|(data, _)| {
            if data.len() == 8 {
                Some(u64::from_le_bytes(data.try_into().unwrap()))
            } else {
                None
            }
        })
    }

    /// Fast path for JSON values under 1KB
    #[inline(always)]
    pub fn get_json<T: for<'de> Deserialize<'de>>(cache: &FastPathCache, key: &str) -> Option<T> {
        cache
            .get_small(key)
            .and_then(|(data, _)| serde_json::from_slice(&data).ok())
    }

    /// Fast path to put string
    #[inline(always)]
    pub fn put_string(
        cache: &FastPathCache,
        key: String,
        value: &str,
        ttl: Option<Duration>,
    ) -> bool {
        let metadata = CacheMetadata {
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: ttl.map(|d| SystemTime::now() + d),
            size_bytes: value.len() as u64,
            access_count: 0,
            content_hash: String::new(), // Skip for fast path
            cache_version: 3,
        };

        cache.put_small(key, value.as_bytes().to_vec(), metadata)
    }

    /// Fast path to put boolean
    #[inline(always)]
    pub fn put_bool(
        cache: &FastPathCache,
        key: String,
        value: bool,
        ttl: Option<Duration>,
    ) -> bool {
        let metadata = CacheMetadata {
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: ttl.map(|d| SystemTime::now() + d),
            size_bytes: 1,
            access_count: 0,
            content_hash: String::new(),
            cache_version: 3,
        };

        cache.put_small(key, vec![if value { 1 } else { 0 }], metadata)
    }
}

/// Batch get operations for improved throughput
pub struct BatchGet<'a> {
    keys: Vec<&'a str>,
    results: Vec<Option<Vec<u8>>>,
}

impl<'a> BatchGet<'a> {
    pub fn new(capacity: usize) -> Self {
        Self {
            keys: Vec::with_capacity(capacity),
            results: Vec::with_capacity(capacity),
        }
    }

    #[inline(always)]
    pub fn add_key(&mut self, key: &'a str) {
        self.keys.push(key);
    }

    pub async fn execute<F>(&mut self, getter: F) -> Vec<Option<Vec<u8>>>
    where
        F: Fn(&str) -> Option<Vec<u8>>,
    {
        // Prefetch all keys
        for key in &self.keys {
            self.results.push(getter(key));
        }

        std::mem::take(&mut self.results)
    }
}

/// Inline cache for extremely hot values
pub struct InlineCache<const N: usize> {
    entries: [(Option<String>, Option<Vec<u8>>); N],
    index: usize,
}

impl<const N: usize> InlineCache<N> {
    pub const fn new() -> Self {
        const EMPTY: (Option<String>, Option<Vec<u8>>) = (None, None);
        Self {
            entries: [EMPTY; N],
            index: 0,
        }
    }

    #[inline(always)]
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        for (k, v) in &self.entries {
            if let (Some(k), Some(v)) = (k, v) {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }

    #[inline(always)]
    pub fn put(&mut self, key: String, value: Vec<u8>) {
        self.entries[self.index] = (Some(key), Some(value));
        self.index = (self.index + 1) % N;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_path_small_values() {
        let cache = FastPathCache::new(1024, 100);

        // Test string fast path
        assert!(specialized::put_string(
            &cache,
            "key1".to_string(),
            "small value",
            None
        ));

        assert_eq!(
            specialized::get_string(&cache, "key1"),
            Some("small value".to_string())
        );

        // Test bool fast path
        assert!(specialized::put_bool(
            &cache,
            "flag".to_string(),
            true,
            None
        ));

        assert_eq!(specialized::get_bool(&cache, "flag"), Some(true));
    }

    #[test]
    fn test_inline_cache() {
        let mut cache = InlineCache::<4>::new();

        cache.put("key1".to_string(), b"value1".to_vec());
        cache.put("key2".to_string(), b"value2".to_vec());

        assert_eq!(cache.get("key1"), Some(b"value1".as_ref()));
        assert_eq!(cache.get("key2"), Some(b"value2".as_ref()));
        assert_eq!(cache.get("key3"), None);

        // Test wraparound
        cache.put("key3".to_string(), b"value3".to_vec());
        cache.put("key4".to_string(), b"value4".to_vec());
        cache.put("key5".to_string(), b"value5".to_vec()); // Overwrites key1

        assert_eq!(cache.get("key1"), None); // Evicted
        assert_eq!(cache.get("key5"), Some(b"value5".as_ref()));
    }

    #[test]
    fn test_fast_path_expiration() {
        let cache = FastPathCache::new(1024, 100);

        // Put with short TTL
        let metadata = CacheMetadata {
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: Some(SystemTime::now() - Duration::from_secs(1)), // Already expired
            size_bytes: 5,
            access_count: 0,
            content_hash: String::new(),
            cache_version: 3,
        };

        cache.put_small("expired".to_string(), b"value".to_vec(), metadata);

        // Should return None due to expiration
        assert!(cache.get_small("expired").is_none());
    }
}
