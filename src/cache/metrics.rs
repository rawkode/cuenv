//! Cache metrics collection and tracking
//!
//! This module provides comprehensive metrics for cache operations including
//! hit rates, latencies, storage efficiency, and access patterns.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Core cache metrics structure for tracking performance and behavior
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    inner: Arc<MetricsInner>,
}

#[derive(Debug)]
struct MetricsInner {
    // Basic counters
    hits: AtomicU64,
    misses: AtomicU64,
    puts: AtomicU64,
    deletes: AtomicU64,
    evictions: AtomicU64,

    // Size metrics
    current_size: AtomicUsize,
    max_size: AtomicUsize,

    // Performance metrics
    total_hit_latency_ns: AtomicU64,
    total_miss_latency_ns: AtomicU64,
    total_put_latency_ns: AtomicU64,

    // Error tracking
    errors: AtomicU64,

    // Advanced metrics
    compression_ratio: RwLock<f64>,
    access_patterns: RwLock<HashMap<String, u64>>,
    start_time: Instant,
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

    /// Record a cache hit
    pub fn record_hit(&self, latency: Duration) {
        self.inner.hits.fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_hit_latency_ns
            .fetch_add(latency.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a cache miss
    pub fn record_miss(&self, latency: Duration) {
        self.inner.misses.fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_miss_latency_ns
            .fetch_add(latency.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a cache put operation
    pub fn record_put(&self, latency: Duration) {
        self.inner.puts.fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_put_latency_ns
            .fetch_add(latency.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a cache delete operation
    pub fn record_delete(&self) {
        self.inner.deletes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an eviction
    pub fn record_eviction(&self) {
        self.inner.evictions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an error
    pub fn record_error(&self) {
        self.inner.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Update current cache size
    pub fn update_size(&self, size: usize) {
        self.inner.current_size.store(size, Ordering::Relaxed);

        // Update max size if needed
        let mut max = self.inner.max_size.load(Ordering::Relaxed);
        while size > max {
            match self.inner.max_size.compare_exchange_weak(
                max,
                size,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => max = current,
            }
        }
    }

    /// Update compression ratio
    pub fn update_compression_ratio(&self, ratio: f64) {
        if let Ok(mut cr) = self.inner.compression_ratio.write() {
            *cr = ratio;
        }
    }

    /// Record an access pattern
    pub fn record_access_pattern(&self, pattern: String) {
        if let Ok(mut patterns) = self.inner.access_patterns.write() {
            *patterns.entry(pattern).or_insert(0) += 1;
        }
    }

    /// Get current hit rate
    pub fn hit_rate(&self) -> f64 {
        let hits = self.inner.hits.load(Ordering::Relaxed);
        let misses = self.inner.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        }
    }

    /// Get average hit latency
    pub fn avg_hit_latency(&self) -> Duration {
        let hits = self.inner.hits.load(Ordering::Relaxed);
        let total_ns = self.inner.total_hit_latency_ns.load(Ordering::Relaxed);

        if hits > 0 {
            Duration::from_nanos(total_ns / hits)
        } else {
            Duration::ZERO
        }
    }

    /// Get average miss latency
    pub fn avg_miss_latency(&self) -> Duration {
        let misses = self.inner.misses.load(Ordering::Relaxed);
        let total_ns = self.inner.total_miss_latency_ns.load(Ordering::Relaxed);

        if misses > 0 {
            Duration::from_nanos(total_ns / misses)
        } else {
            Duration::ZERO
        }
    }

    /// Get average put latency
    pub fn avg_put_latency(&self) -> Duration {
        let puts = self.inner.puts.load(Ordering::Relaxed);
        let total_ns = self.inner.total_put_latency_ns.load(Ordering::Relaxed);

        if puts > 0 {
            Duration::from_nanos(total_ns / puts)
        } else {
            Duration::ZERO
        }
    }

    /// Get current cache size
    pub fn current_size(&self) -> usize {
        self.inner.current_size.load(Ordering::Relaxed)
    }

    /// Get maximum cache size seen
    pub fn max_size(&self) -> usize {
        self.inner.max_size.load(Ordering::Relaxed)
    }

    /// Get total operations count
    pub fn total_operations(&self) -> u64 {
        let hits = self.inner.hits.load(Ordering::Relaxed);
        let misses = self.inner.misses.load(Ordering::Relaxed);
        let puts = self.inner.puts.load(Ordering::Relaxed);
        let deletes = self.inner.deletes.load(Ordering::Relaxed);

        hits + misses + puts + deletes
    }

    /// Get eviction count
    pub fn evictions(&self) -> u64 {
        self.inner.evictions.load(Ordering::Relaxed)
    }

    /// Get error count
    pub fn errors(&self) -> u64 {
        self.inner.errors.load(Ordering::Relaxed)
    }

    /// Get uptime
    pub fn uptime(&self) -> Duration {
        self.inner.start_time.elapsed()
    }

    /// Get compression ratio
    pub fn compression_ratio(&self) -> f64 {
        self.inner
            .compression_ratio
            .read()
            .copied()
            .unwrap_or(1.0)
    }

    /// Get top access patterns
    pub fn top_access_patterns(&self, limit: usize) -> Vec<(String, u64)> {
        if let Ok(patterns) = self.inner.access_patterns.read() {
            let mut sorted: Vec<_> = patterns.iter().map(|(k, v)| (k.clone(), *v)).collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            sorted.truncate(limit);
            sorted
        } else {
            Vec::new()
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.inner.hits.store(0, Ordering::Relaxed);
        self.inner.misses.store(0, Ordering::Relaxed);
        self.inner.puts.store(0, Ordering::Relaxed);
        self.inner.deletes.store(0, Ordering::Relaxed);
        self.inner.evictions.store(0, Ordering::Relaxed);
        self.inner.errors.store(0, Ordering::Relaxed);
        self.inner.current_size.store(0, Ordering::Relaxed);
        self.inner.max_size.store(0, Ordering::Relaxed);
        self.inner.total_hit_latency_ns.store(0, Ordering::Relaxed);
        self.inner.total_miss_latency_ns.store(0, Ordering::Relaxed);
        self.inner.total_put_latency_ns.store(0, Ordering::Relaxed);

        if let Ok(mut ratio) = self.inner.compression_ratio.write() {
            *ratio = 1.0;
        }

        if let Ok(mut patterns) = self.inner.access_patterns.write() {
            patterns.clear();
        }
    }
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// A snapshot of cache metrics at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub puts: u64,
    pub deletes: u64,
    pub evictions: u64,
    pub errors: u64,
    pub hit_rate: f64,
    pub current_size: usize,
    pub max_size: usize,
    pub avg_hit_latency: Duration,
    pub avg_miss_latency: Duration,
    pub avg_put_latency: Duration,
    pub compression_ratio: f64,
    pub uptime: Duration,
    pub timestamp: Instant,
}

impl CacheMetrics {
    /// Take a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            hits: self.inner.hits.load(Ordering::Relaxed),
            misses: self.inner.misses.load(Ordering::Relaxed),
            puts: self.inner.puts.load(Ordering::Relaxed),
            deletes: self.inner.deletes.load(Ordering::Relaxed),
            evictions: self.inner.evictions.load(Ordering::Relaxed),
            errors: self.inner.errors.load(Ordering::Relaxed),
            hit_rate: self.hit_rate(),
            current_size: self.current_size(),
            max_size: self.max_size(),
            avg_hit_latency: self.avg_hit_latency(),
            avg_miss_latency: self.avg_miss_latency(),
            avg_put_latency: self.avg_put_latency(),
            compression_ratio: self.compression_ratio(),
            uptime: self.uptime(),
            timestamp: Instant::now(),
        }
    }
}
