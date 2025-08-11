//! LRU (Least Recently Used) eviction policy implementation

use crate::eviction::traits::EvictionPolicy;
use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

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

        let order = self.access_order.try_lock()?;

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
