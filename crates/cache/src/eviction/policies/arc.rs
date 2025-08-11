//! ARC (Adaptive Replacement Cache) eviction policy implementation

use crate::eviction::traits::EvictionPolicy;
use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

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
