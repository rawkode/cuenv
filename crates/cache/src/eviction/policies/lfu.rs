//! LFU (Least Frequently Used) eviction policy implementation

use crate::eviction::traits::EvictionPolicy;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// LFU (Least Frequently Used) eviction policy
pub struct LfuPolicy {
    /// Frequency counts per key
    frequencies: DashMap<String, u64>,
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
            sizes: DashMap::new(),
            total_size: AtomicU64::new(0),
            max_memory,
        }
    }
}

impl EvictionPolicy for LfuPolicy {
    fn on_access(&self, key: &str, _size: u64) {
        self.frequencies
            .entry(key.to_string())
            .and_modify(|f| *f += 1)
            .or_insert(1);
    }

    fn on_insert(&self, key: &str, size: u64) {
        self.sizes.insert(key.to_string(), size);
        self.total_size.fetch_add(size, Ordering::AcqRel);
        self.frequencies.insert(key.to_string(), 1);
    }

    fn on_remove(&self, key: &str, size: u64) {
        self.frequencies.remove(key);
        self.sizes.remove(key);
        self.total_size.fetch_sub(size, Ordering::AcqRel);
    }

    fn next_eviction(&self) -> Option<String> {
        // Check if eviction is needed
        if self.memory_usage() <= self.max_memory {
            return None;
        }

        // Lock-free O(n) scan over frequencies to find the minimum.
        let mut candidate: Option<(String, u64)> = None;

        for r in self.frequencies.iter() {
            let key = r.key();
            let freq = *r.value();

            // Ensure the key hasn't been removed (still has a size recorded)
            if self.sizes.contains_key(key) {
                match candidate {
                    Some((_, best_freq)) if freq < best_freq => {
                        candidate = Some((key.clone(), freq));
                    }
                    None => {
                        candidate = Some((key.clone(), freq));
                    }
                    _ => {}
                }
            }
        }

        candidate.map(|(k, _)| k)
    }

    fn clear(&self) {
        self.frequencies.clear();
        self.sizes.clear();
        self.total_size.store(0, Ordering::Release);
    }

    fn memory_usage(&self) -> u64 {
        self.total_size.load(Ordering::Acquire)
    }
}
