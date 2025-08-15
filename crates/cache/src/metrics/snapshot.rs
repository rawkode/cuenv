//! Metrics snapshot functionality
//!
//! This module provides snapshot capabilities for capturing
//! a point-in-time view of all metrics.

use super::core::CacheMetrics;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

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