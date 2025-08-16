//! Core cache metrics structures and basic operations
//!
//! This module defines the fundamental metric tracking structures
//! and provides basic recording operations.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Core cache metrics structure for tracking performance and behavior
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    pub(crate) inner: Arc<MetricsInner>,
}

#[derive(Debug)]
pub(crate) struct MetricsInner {
    // Basic counters
    pub(crate) hits: AtomicU64,
    pub(crate) misses: AtomicU64,
    pub(crate) puts: AtomicU64,
    pub(crate) deletes: AtomicU64,
    pub(crate) evictions: AtomicU64,

    // Size metrics
    pub(crate) current_size: AtomicUsize,
    pub(crate) max_size: AtomicUsize,

    // Performance metrics
    pub(crate) total_hit_latency_ns: AtomicU64,
    pub(crate) total_miss_latency_ns: AtomicU64,
    pub(crate) total_put_latency_ns: AtomicU64,

    // Error tracking
    pub(crate) errors: AtomicU64,

    // Advanced metrics
    pub(crate) compression_ratio: RwLock<f64>,
    pub(crate) access_patterns: RwLock<HashMap<String, u64>>,
    pub(crate) start_time: Instant,
}

impl CacheMetrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
                puts: AtomicU64::new(0),
                deletes: AtomicU64::new(0),
                evictions: AtomicU64::new(0),
                current_size: AtomicUsize::new(0),
                max_size: AtomicUsize::new(0),
                total_hit_latency_ns: AtomicU64::new(0),
                total_miss_latency_ns: AtomicU64::new(0),
                total_put_latency_ns: AtomicU64::new(0),
                errors: AtomicU64::new(0),
                compression_ratio: RwLock::new(1.0),
                access_patterns: RwLock::new(HashMap::new()),
                start_time: Instant::now(),
            }),
        }
    }
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}
