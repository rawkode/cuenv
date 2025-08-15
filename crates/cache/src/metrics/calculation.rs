//! Metrics calculation and retrieval operations
//!
//! This module provides methods for calculating derived metrics
//! like hit rates, average latencies, and other computed values.

use super::core::CacheMetrics;
use std::sync::atomic::Ordering;
use std::time::Duration;

impl CacheMetrics {
    /// Get current hit rate
    pub fn hit_rate(&self) -> f64 {
        let hits = self.inner.hits.load(Ordering::Relaxed) as f64;
        let misses = self.inner.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;

        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }

    /// Get average hit latency
    pub fn avg_hit_latency(&self) -> Duration {
        let hits = self.inner.hits.load(Ordering::Relaxed);
        let total_latency_ns = self.inner.total_hit_latency_ns.load(Ordering::Relaxed);

        if hits == 0 {
            Duration::ZERO
        } else {
            Duration::from_nanos(total_latency_ns.div_ceil(hits))
        }
    }

    /// Get average miss latency
    pub fn avg_miss_latency(&self) -> Duration {
        let misses = self.inner.misses.load(Ordering::Relaxed);
        let total_latency_ns = self.inner.total_miss_latency_ns.load(Ordering::Relaxed);

        if misses == 0 {
            Duration::ZERO
        } else {
            Duration::from_nanos(total_latency_ns.div_ceil(misses))
        }
    }

    /// Get average put latency
    pub fn avg_put_latency(&self) -> Duration {
        let puts = self.inner.puts.load(Ordering::Relaxed);
        let total_latency_ns = self.inner.total_put_latency_ns.load(Ordering::Relaxed);

        if puts == 0 {
            Duration::ZERO
        } else {
            Duration::from_nanos(total_latency_ns.div_ceil(puts))
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
        self.inner.hits.load(Ordering::Relaxed)
            + self.inner.misses.load(Ordering::Relaxed)
            + self.inner.puts.load(Ordering::Relaxed)
            + self.inner.deletes.load(Ordering::Relaxed)
    }

    /// Get eviction count
    pub fn eviction_count(&self) -> u64 {
        self.inner.evictions.load(Ordering::Relaxed)
    }

    /// Get error count
    pub fn error_count(&self) -> u64 {
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
            .map(|ratio| *ratio)
            .unwrap_or(1.0)
    }

    /// Get top access patterns
    pub fn top_access_patterns(&self, limit: usize) -> Vec<(String, u64)> {
        let patterns = self.inner.access_patterns.read();
        match patterns {
            Ok(patterns) => {
                let mut sorted_patterns: Vec<_> = patterns.iter().collect();
                sorted_patterns.sort_by(|a, b| b.1.cmp(a.1));
                sorted_patterns
                    .into_iter()
                    .take(limit)
                    .map(|(k, v)| (k.clone(), *v))
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }
}