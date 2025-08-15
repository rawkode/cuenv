//! Metrics collection and recording operations
//!
//! This module provides methods for recording cache operations
//! and updating various metrics counters.

use super::core::CacheMetrics;
use std::sync::atomic::Ordering;
use std::time::Duration;

impl CacheMetrics {
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
    pub fn update_size(&self, new_size: usize) {
        self.inner.current_size.store(new_size, Ordering::Relaxed);

        // Update max size if needed
        let current_max = self.inner.max_size.load(Ordering::Relaxed);
        if new_size > current_max {
            // Try to update max_size, but don't worry if another thread beats us
            let _ = self.inner.max_size.compare_exchange_weak(
                current_max,
                new_size,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
    }

    /// Update compression ratio
    pub fn update_compression_ratio(&self, ratio: f64) {
        if let Ok(mut compression_ratio) = self.inner.compression_ratio.write() {
            *compression_ratio = ratio;
        }
    }

    /// Record an access pattern
    pub fn record_access_pattern(&self, pattern: &str) {
        if let Ok(mut patterns) = self.inner.access_patterns.write() {
            *patterns.entry(pattern.to_string()).or_insert(0) += 1;
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.inner.hits.store(0, Ordering::Relaxed);
        self.inner.misses.store(0, Ordering::Relaxed);
        self.inner.puts.store(0, Ordering::Relaxed);
        self.inner.deletes.store(0, Ordering::Relaxed);
        self.inner.evictions.store(0, Ordering::Relaxed);
        self.inner.current_size.store(0, Ordering::Relaxed);
        self.inner.max_size.store(0, Ordering::Relaxed);
        self.inner
            .total_hit_latency_ns
            .store(0, Ordering::Relaxed);
        self.inner
            .total_miss_latency_ns
            .store(0, Ordering::Relaxed);
        self.inner
            .total_put_latency_ns
            .store(0, Ordering::Relaxed);
        self.inner.errors.store(0, Ordering::Relaxed);

        if let Ok(mut compression_ratio) = self.inner.compression_ratio.write() {
            *compression_ratio = 1.0;
        }

        if let Ok(mut patterns) = self.inner.access_patterns.write() {
            patterns.clear();
        }
    }
}