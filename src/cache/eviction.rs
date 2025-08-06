//! Eviction policies for cache memory management
//!
//! Implements LRU, LFU, and ARC eviction strategies with
//! production-grade performance and correctness.

#![allow(dead_code)]

use crate::cache::errors::{CacheError, RecoveryHint, Result};
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Eviction policy trait
pub trait EvictionPolicy: Send + Sync {
    /// Record access to a key
    fn on_access(&self, key: &str, size: u64);

    /// Record insertion of a key
    fn on_insert(&self, key: &str, size: u64);

    /// Record removal of a key
    fn on_remove(&self, key: &str, size: u64);

    /// Get next key to evict
    fn next_eviction(&self) -> Option<String>;

    /// Clear all tracking data
    fn clear(&self);

    /// Get current memory usage
    fn memory_usage(&self) -> u64;
}

/// LRU (Least Recently Used) eviction policy
pub struct LruPolicy {
    /// Access order tracking
    access_order: Mutex<VecDeque<String>>,
    /// Size tracking
    sizes: DashMap<String, u64>,
    /// Total memory usage
    total_size: AtomicU64,
    /// Maximum memory allowed
    max_memory: u64,
}

impl LruPolicy {
    pub fn new(max_memory: u64) -> Self {
        Self {
            access_order: Mutex::new(VecDeque::new()),
            sizes: DashMap::new(),
            total_size: AtomicU64::new(0),
            max_memory,
        }
    }
}

impl EvictionPolicy for LruPolicy {
    fn on_access(&self, key: &str, _size: u64) {
        let mut order = match self.access_order.try_lock() {
            Some(guard) => guard,
            None => return, // Skip if contended
        };

        // Move to front (most recently used)
        order.retain(|k| k != key);
        order.push_back(key.to_string());
    }

    fn on_insert(&self, key: &str, size: u64) {
        self.sizes.insert(key.to_string(), size);
        self.total_size.fetch_add(size, Ordering::AcqRel);

        let mut order = match self.access_order.try_lock() {
            Some(guard) => guard,
            None => return,
        };

        order.push_back(key.to_string());
    }

    fn on_remove(&self, key: &str, size: u64) {
        self.sizes.remove(key);
        self.total_size.fetch_sub(size, Ordering::AcqRel);

        let mut order = match self.access_order.try_lock() {
            Some(guard) => guard,
            None => return,
        };

        order.retain(|k| k != key);
    }

    fn next_eviction(&self) -> Option<String> {
        if self.memory_usage() <= self.max_memory {
            return None;
        }

        #[allow(clippy::question_mark)]
        let order = match self.access_order.try_lock() {
            Some(guard) => guard,
            None => return None,
        };

        order.front().cloned()
    }

    fn clear(&self) {
        self.sizes.clear();
        self.total_size.store(0, Ordering::Release);

        if let Some(mut order) = self.access_order.try_lock() {
            order.clear();
        }
    }

    fn memory_usage(&self) -> u64 {
        self.total_size.load(Ordering::Acquire)
    }
}

/// LFU (Least Frequently Used) eviction policy
pub struct LfuPolicy {
    /// Frequency counts
    frequencies: DashMap<String, u64>,
    /// Frequency to keys mapping
    freq_map: RwLock<BTreeMap<u64, Vec<String>>>,
    /// Size tracking
    sizes: DashMap<String, u64>,
    /// Total memory usage
    total_size: AtomicU64,
    /// Maximum memory allowed
    max_memory: u64,
}

impl LfuPolicy {
    pub fn new(max_memory: u64) -> Self {
        Self {
            frequencies: DashMap::new(),
            freq_map: RwLock::new(BTreeMap::new()),
            sizes: DashMap::new(),
            total_size: AtomicU64::new(0),
            max_memory,
        }
    }

    fn update_frequency(&self, key: &str, old_freq: u64, new_freq: u64) {
        let mut freq_map = match self.freq_map.try_write() {
            Some(guard) => guard,
            None => return,
        };

        // Remove from old frequency list
        if old_freq > 0 {
            if let Some(keys) = freq_map.get_mut(&old_freq) {
                keys.retain(|k| k != key);
                if keys.is_empty() {
                    freq_map.remove(&old_freq);
                }
            }
        }

        // Add to new frequency list
        freq_map
            .entry(new_freq)
            .or_insert_with(Vec::new)
            .push(key.to_string());
    }
}

impl EvictionPolicy for LfuPolicy {
    fn on_access(&self, key: &str, _size: u64) {
        let old_freq = self.frequencies.get(key).copied().unwrap_or(0);
        let new_freq = old_freq + 1;

        self.frequencies.insert(key.to_string(), new_freq);
        self.update_frequency(key, old_freq, new_freq);
    }

    fn on_insert(&self, key: &str, size: u64) {
        self.sizes.insert(key.to_string(), size);
        self.total_size.fetch_add(size, Ordering::AcqRel);
        self.frequencies.insert(key.to_string(), 1);
        self.update_frequency(key, 0, 1);
    }

    fn on_remove(&self, key: &str, size: u64) {
        if let Some((_, freq)) = self.frequencies.remove(key) {
            self.update_frequency(key, freq, 0);
        }

        self.sizes.remove(key);
        self.total_size.fetch_sub(size, Ordering::AcqRel);
    }

    fn next_eviction(&self) -> Option<String> {
        if self.memory_usage() <= self.max_memory {
            return None;
        }

        #[allow(clippy::question_mark)]
        let freq_map = match self.freq_map.try_read() {
            Some(guard) => guard,
            None => return None,
        };

        // Find key with lowest frequency
        freq_map
            .iter()
            .next()
            .and_then(|(_, keys)| keys.first().cloned())
    }

    fn clear(&self) {
        self.frequencies.clear();
        self.sizes.clear();
        self.total_size.store(0, Ordering::Release);

        if let Some(mut freq_map) = self.freq_map.try_write() {
            freq_map.clear();
        }
    }

    fn memory_usage(&self) -> u64 {
        self.total_size.load(Ordering::Acquire)
    }
}

/// ARC (Adaptive Replacement Cache) eviction policy
pub struct ArcPolicy {
    /// Target size for frequently used entries
    p: AtomicUsize,
    /// Maximum cache size
    c: usize,
    /// T1: recent cache entries
    t1: Mutex<VecDeque<String>>,
    /// T2: frequent cache entries  
    t2: Mutex<VecDeque<String>>,
    /// B1: ghost entries recently evicted from T1
    b1: Mutex<VecDeque<String>>,
    /// B2: ghost entries recently evicted from T2
    b2: Mutex<VecDeque<String>>,
    /// Size tracking
    sizes: DashMap<String, u64>,
    /// Total memory usage
    total_size: AtomicU64,
    /// Maximum memory allowed
    max_memory: u64,
}

impl ArcPolicy {
    pub fn new(max_memory: u64) -> Self {
        let c = (max_memory / 4096) as usize; // Assume 4KB average entry

        Self {
            p: AtomicUsize::new(c / 2),
            c,
            t1: Mutex::new(VecDeque::new()),
            t2: Mutex::new(VecDeque::new()),
            b1: Mutex::new(VecDeque::new()),
            b2: Mutex::new(VecDeque::new()),
            sizes: DashMap::new(),
            total_size: AtomicU64::new(0),
            max_memory,
        }
    }

    fn adapt(&self, in_b1: bool) {
        let p = self.p.load(Ordering::Acquire);

        if in_b1 {
            // Increase p (favor recency)
            let delta = 1.max(self.b2.lock().len() / self.b1.lock().len());
            let new_p = (p + delta).min(self.c);
            self.p.store(new_p, Ordering::Release);
        } else {
            // Decrease p (favor frequency)
            let delta = 1.max(self.b1.lock().len() / self.b2.lock().len());
            let new_p = p.saturating_sub(delta);
            self.p.store(new_p, Ordering::Release);
        }
    }
}

impl EvictionPolicy for ArcPolicy {
    fn on_access(&self, key: &str, _size: u64) {
        let mut t1 = match self.t1.try_lock() {
            Some(guard) => guard,
            None => return,
        };

        let mut t2 = match self.t2.try_lock() {
            Some(guard) => guard,
            None => return,
        };

        // Check if in T1 (move to T2)
        if let Some(pos) = t1.iter().position(|k| k == key) {
            t1.remove(pos);
            t2.push_back(key.to_string());
            return;
        }

        // Check if in T2 (move to front)
        if let Some(pos) = t2.iter().position(|k| k == key) {
            t2.remove(pos);
            t2.push_back(key.to_string());
        }
    }

    fn on_insert(&self, key: &str, size: u64) {
        self.sizes.insert(key.to_string(), size);
        self.total_size.fetch_add(size, Ordering::AcqRel);

        // Check ghost lists
        let in_b1 = self.b1.lock().contains(&key.to_string());
        let in_b2 = self.b2.lock().contains(&key.to_string());

        if in_b1 || in_b2 {
            self.adapt(in_b1);

            // Remove from ghost list
            if in_b1 {
                self.b1.lock().retain(|k| k != key);
            } else {
                self.b2.lock().retain(|k| k != key);
            }

            // Add to T2 (frequent)
            self.t2.lock().push_back(key.to_string());
        } else {
            // New entry - add to T1
            self.t1.lock().push_back(key.to_string());
        }
    }

    fn on_remove(&self, key: &str, size: u64) {
        self.sizes.remove(key);
        self.total_size.fetch_sub(size, Ordering::AcqRel);

        // Remove from all lists
        self.t1.lock().retain(|k| k != key);
        self.t2.lock().retain(|k| k != key);
        self.b1.lock().retain(|k| k != key);
        self.b2.lock().retain(|k| k != key);
    }

    fn next_eviction(&self) -> Option<String> {
        if self.memory_usage() <= self.max_memory {
            return None;
        }

        let p = self.p.load(Ordering::Acquire);
        let t1_len = self.t1.lock().len();

        // Evict from T1 or T2 based on p
        if t1_len > 0 && (t1_len > p || self.t2.lock().is_empty()) {
            // Evict from T1
            let mut t1 = self.t1.lock();
            let key = t1.pop_front();

            // Add to B1 ghost list
            if let Some(ref k) = key {
                self.b1.lock().push_back(k.clone());
            }

            key
        } else {
            // Evict from T2
            let mut t2 = self.t2.lock();
            let key = t2.pop_front();

            // Add to B2 ghost list
            if let Some(ref k) = key {
                self.b2.lock().push_back(k.clone());
            }

            key
        }
    }

    fn clear(&self) {
        self.sizes.clear();
        self.total_size.store(0, Ordering::Release);
        self.p.store(self.c / 2, Ordering::Release);

        self.t1.lock().clear();
        self.t2.lock().clear();
        self.b1.lock().clear();
        self.b2.lock().clear();
    }

    fn memory_usage(&self) -> u64 {
        self.total_size.load(Ordering::Acquire)
    }
}

/// Eviction policy factory
pub fn create_eviction_policy(
    policy_type: &str,
    max_memory: u64,
) -> Result<Box<dyn EvictionPolicy>> {
    match policy_type.to_lowercase().as_str() {
        "lru" => Ok(Box::new(LruPolicy::new(max_memory))),
        "lfu" => Ok(Box::new(LfuPolicy::new(max_memory))),
        "arc" => Ok(Box::new(ArcPolicy::new(max_memory))),
        _ => Err(CacheError::Configuration {
            message: format!("Unknown eviction policy: {policy_type}"),
            recovery_hint: RecoveryHint::UseDefault {
                value: "lru".to_string(),
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_eviction() {
        let policy = LruPolicy::new(1000);

        // Insert entries
        policy.on_insert("a", 100);
        policy.on_insert("b", 200);
        policy.on_insert("c", 300);
        policy.on_insert("d", 500); // Total: 1100, over limit

        // Should evict 'a' (least recently used)
        assert_eq!(policy.next_eviction(), Some("a".to_string()));

        // Access 'b' to make it more recent
        policy.on_access("b", 200);
        policy.on_remove("a", 100); // Total: 1000

        policy.on_insert("e", 200); // Total: 1200, over limit

        // Should evict 'c' (now least recently used)
        assert_eq!(policy.next_eviction(), Some("c".to_string()));
    }

    #[test]
    fn test_lfu_eviction() {
        let policy = LfuPolicy::new(1000);

        // Insert and access entries
        policy.on_insert("a", 300);
        policy.on_insert("b", 300);
        policy.on_insert("c", 300);

        // Access 'a' and 'b' more frequently
        policy.on_access("a", 300);
        policy.on_access("a", 300);
        policy.on_access("b", 300);

        policy.on_insert("d", 300); // Total: 1200, over limit

        // Should evict 'c' (least frequently used)
        assert_eq!(policy.next_eviction(), Some("c".to_string()));
    }

    #[test]
    fn test_arc_adaptation() {
        let policy = ArcPolicy::new(1000);

        // Insert entries
        policy.on_insert("a", 250);
        policy.on_insert("b", 250);
        policy.on_insert("c", 250);
        policy.on_insert("d", 250);

        // Access 'a' and 'b' to move to T2
        policy.on_access("a", 250);
        policy.on_access("b", 250);

        policy.on_insert("e", 250); // Total: 1250, over limit

        // Should evict from T1 (c or d)
        let evicted = policy.next_eviction().unwrap();
        assert!(evicted == "c" || evicted == "d");
    }
}
