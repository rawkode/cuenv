//! Core fast path cache implementation

use crate::traits::CacheMetadata;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

/// Fast path cache for small, frequently accessed values
pub struct FastPathCache {
    /// Small value cache (up to 1KB)
    pub(crate) small_values: DashMap<String, Arc<SmallValue>>,
    /// Maximum size for small values
    pub(crate) small_value_threshold: usize,
    /// LRU tracking for eviction
    pub(crate) lru_tracker: Arc<RwLock<LruTracker>>,
}

pub(crate) struct SmallValue {
    pub data: Vec<u8>,
    pub metadata: CacheMetadata,
    pub last_access: RwLock<Instant>,
}

pub(crate) struct LruTracker {
    /// Access order tracking
    pub access_order: Vec<String>,
    /// Maximum entries to track
    pub max_entries: usize,
}

impl FastPathCache {
    pub fn new(small_value_threshold: usize, max_entries: usize) -> Self {
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
        if let Some(mut tracker) = self.lru_tracker.try_write() {
            tracker.access_order.retain(|k| k != &key);
            tracker.access_order.push(key);
        }

        true
    }

    /// Evict least recently used entry
    fn evict_lru(&self) {
        if let Some(mut tracker) = self.lru_tracker.try_write() {
            if let Some(key) = tracker.access_order.first() {
                let key = key.clone();
                tracker.access_order.remove(0);
                self.small_values.remove(&key);
            }
        }
    }

    /// Check if a key exists in the fast-path cache
    #[inline(always)]
    pub fn contains_small(&self, key: &str) -> bool {
        if let Some(entry) = self.small_values.get(key) {
            // Check expiration
            if let Some(expires_at) = entry.metadata.expires_at {
                if expires_at <= SystemTime::now() {
                    // Expired - remove and return false
                    drop(entry);
                    self.small_values.remove(key);
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// Remove a small-value entry
    ///
    /// Returns `true` if an entry existed and was removed.
    pub fn remove_small(&self, key: &str) -> bool {
        let removed = self.small_values.remove(key).is_some();

        if removed {
            // Keep LRU tracker in sync
            if let Some(mut tracker) = self.lru_tracker.try_write() {
                tracker.access_order.retain(|k| k != key);
            }
        }

        removed
    }

    /// Clear all fast path entries
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.small_values.clear();
        if let Some(mut tracker) = self.lru_tracker.try_write() {
            tracker.access_order.clear();
        }
    }
}
