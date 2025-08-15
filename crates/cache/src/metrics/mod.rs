//! Cache metrics collection and tracking
//!
//! This module provides comprehensive metrics for cache operations including
//! hit rates, latencies, storage efficiency, and access patterns.

pub mod endpoint;

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
            .map(|v| *v)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn test_cache_metrics_creation() {
        let metrics = CacheMetrics::new();

        assert_eq!(metrics.hit_rate(), 0.0);
        assert_eq!(metrics.current_size(), 0);
        assert_eq!(metrics.max_size(), 0);
        assert_eq!(metrics.total_operations(), 0);
        assert_eq!(metrics.evictions(), 0);
        assert_eq!(metrics.errors(), 0);
        assert_eq!(metrics.compression_ratio(), 1.0);
        assert_eq!(metrics.avg_hit_latency(), Duration::ZERO);
        assert_eq!(metrics.avg_miss_latency(), Duration::ZERO);
        assert_eq!(metrics.avg_put_latency(), Duration::ZERO);
    }

    #[test]
    fn test_default_implementation() {
        let metrics1 = CacheMetrics::new();
        let metrics2 = CacheMetrics::default();

        assert_eq!(metrics1.hit_rate(), metrics2.hit_rate());
        assert_eq!(metrics1.current_size(), metrics2.current_size());
        assert_eq!(metrics1.total_operations(), metrics2.total_operations());
    }

    #[test]
    fn test_hit_recording() {
        let metrics = CacheMetrics::new();
        let latency = Duration::from_millis(10);

        metrics.record_hit(latency);
        metrics.record_hit(latency);

        assert_eq!(metrics.inner.hits.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.avg_hit_latency(), latency);
        assert_eq!(metrics.hit_rate(), 1.0); // Only hits, no misses
    }

    #[test]
    fn test_miss_recording() {
        let metrics = CacheMetrics::new();
        let latency = Duration::from_millis(50);

        metrics.record_miss(latency);
        metrics.record_miss(latency);

        assert_eq!(metrics.inner.misses.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.avg_miss_latency(), latency);
        assert_eq!(metrics.hit_rate(), 0.0); // Only misses, no hits
    }

    #[test]
    fn test_put_recording() {
        let metrics = CacheMetrics::new();
        let latency = Duration::from_millis(25);

        metrics.record_put(latency);
        metrics.record_put(latency);

        assert_eq!(metrics.inner.puts.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.avg_put_latency(), latency);
    }

    #[test]
    fn test_delete_recording() {
        let metrics = CacheMetrics::new();

        metrics.record_delete();
        metrics.record_delete();

        assert_eq!(metrics.inner.deletes.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_eviction_recording() {
        let metrics = CacheMetrics::new();

        metrics.record_eviction();
        metrics.record_eviction();

        assert_eq!(metrics.evictions(), 2);
    }

    #[test]
    fn test_error_recording() {
        let metrics = CacheMetrics::new();

        metrics.record_error();
        metrics.record_error();

        assert_eq!(metrics.errors(), 2);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let metrics = CacheMetrics::new();

        // Test with no operations
        assert_eq!(metrics.hit_rate(), 0.0);

        // Test with only hits
        metrics.record_hit(Duration::from_millis(1));
        metrics.record_hit(Duration::from_millis(1));
        assert_eq!(metrics.hit_rate(), 1.0);

        // Test with hits and misses
        metrics.record_miss(Duration::from_millis(1));
        assert_eq!(metrics.hit_rate(), 2.0 / 3.0);

        // Test with only misses (reset first)
        let metrics2 = CacheMetrics::new();
        metrics2.record_miss(Duration::from_millis(1));
        metrics2.record_miss(Duration::from_millis(1));
        assert_eq!(metrics2.hit_rate(), 0.0);
    }

    #[test]
    fn test_size_management() {
        let metrics = CacheMetrics::new();

        // Test initial size
        assert_eq!(metrics.current_size(), 0);
        assert_eq!(metrics.max_size(), 0);

        // Test size updates
        metrics.update_size(100);
        assert_eq!(metrics.current_size(), 100);
        assert_eq!(metrics.max_size(), 100);

        // Test max size tracking
        metrics.update_size(50);
        assert_eq!(metrics.current_size(), 50);
        assert_eq!(metrics.max_size(), 100); // Should remain 100

        metrics.update_size(200);
        assert_eq!(metrics.current_size(), 200);
        assert_eq!(metrics.max_size(), 200); // Should update to 200
    }

    #[test]
    fn test_compression_ratio() {
        let metrics = CacheMetrics::new();

        // Test default compression ratio
        assert_eq!(metrics.compression_ratio(), 1.0);

        // Test updating compression ratio
        metrics.update_compression_ratio(0.5);
        assert_eq!(metrics.compression_ratio(), 0.5);

        metrics.update_compression_ratio(2.0);
        assert_eq!(metrics.compression_ratio(), 2.0);
    }

    #[test]
    fn test_access_patterns() {
        let metrics = CacheMetrics::new();

        // Test recording access patterns
        metrics.record_access_pattern("pattern1".to_string());
        metrics.record_access_pattern("pattern2".to_string());
        metrics.record_access_pattern("pattern1".to_string()); // Duplicate

        let top_patterns = metrics.top_access_patterns(10);
        assert_eq!(top_patterns.len(), 2);

        // Check that pattern1 has count 2 and pattern2 has count 1
        let pattern1_count = top_patterns
            .iter()
            .find(|(p, _)| p == "pattern1")
            .map(|(_, c)| *c);
        let pattern2_count = top_patterns
            .iter()
            .find(|(p, _)| p == "pattern2")
            .map(|(_, c)| *c);

        assert_eq!(pattern1_count, Some(2));
        assert_eq!(pattern2_count, Some(1));

        // Test that patterns are sorted by count (descending)
        assert!(top_patterns[0].1 >= top_patterns[1].1);
    }

    #[test]
    fn test_top_access_patterns_limit() {
        let metrics = CacheMetrics::new();

        // Record many patterns
        for i in 0..10 {
            metrics.record_access_pattern(format!("pattern{}", i));
        }

        let top_3 = metrics.top_access_patterns(3);
        assert_eq!(top_3.len(), 3);

        let all_patterns = metrics.top_access_patterns(20);
        assert_eq!(all_patterns.len(), 10);
    }

    #[test]
    fn test_total_operations() {
        let metrics = CacheMetrics::new();

        assert_eq!(metrics.total_operations(), 0);

        metrics.record_hit(Duration::from_millis(1));
        metrics.record_miss(Duration::from_millis(1));
        metrics.record_put(Duration::from_millis(1));
        metrics.record_delete();

        assert_eq!(metrics.total_operations(), 4);

        // Evictions and errors should not count towards total operations
        metrics.record_eviction();
        metrics.record_error();
        assert_eq!(metrics.total_operations(), 4);
    }

    #[test]
    fn test_latency_calculations() {
        let metrics = CacheMetrics::new();

        // Test with no operations
        assert_eq!(metrics.avg_hit_latency(), Duration::ZERO);
        assert_eq!(metrics.avg_miss_latency(), Duration::ZERO);
        assert_eq!(metrics.avg_put_latency(), Duration::ZERO);

        // Test with operations
        metrics.record_hit(Duration::from_millis(10));
        metrics.record_hit(Duration::from_millis(20));
        assert_eq!(metrics.avg_hit_latency(), Duration::from_millis(15));

        metrics.record_miss(Duration::from_millis(100));
        metrics.record_miss(Duration::from_millis(200));
        assert_eq!(metrics.avg_miss_latency(), Duration::from_millis(150));

        metrics.record_put(Duration::from_millis(30));
        metrics.record_put(Duration::from_millis(60));
        assert_eq!(metrics.avg_put_latency(), Duration::from_millis(45));
    }

    #[test]
    fn test_uptime() {
        let metrics = CacheMetrics::new();

        let start_time = Instant::now();
        thread::sleep(Duration::from_millis(10));
        let uptime = metrics.uptime();

        assert!(uptime >= Duration::from_millis(10));
        assert!(uptime <= Instant::now().duration_since(start_time) + Duration::from_millis(1));
    }

    #[test]
    fn test_reset() {
        let metrics = CacheMetrics::new();

        // Set up some data
        metrics.record_hit(Duration::from_millis(10));
        metrics.record_miss(Duration::from_millis(20));
        metrics.record_put(Duration::from_millis(30));
        metrics.record_delete();
        metrics.record_eviction();
        metrics.record_error();
        metrics.update_size(100);
        metrics.update_compression_ratio(0.5);
        metrics.record_access_pattern("test_pattern".to_string());

        // Verify data is set
        assert!(metrics.total_operations() > 0);
        assert!(metrics.evictions() > 0);
        assert!(metrics.errors() > 0);
        assert!(metrics.current_size() > 0);
        assert!(metrics.max_size() > 0);
        assert_ne!(metrics.compression_ratio(), 1.0);
        assert!(!metrics.top_access_patterns(10).is_empty());

        // Reset and verify everything is cleared
        metrics.reset();

        assert_eq!(metrics.total_operations(), 0);
        assert_eq!(metrics.evictions(), 0);
        assert_eq!(metrics.errors(), 0);
        assert_eq!(metrics.current_size(), 0);
        assert_eq!(metrics.max_size(), 0);
        assert_eq!(metrics.avg_hit_latency(), Duration::ZERO);
        assert_eq!(metrics.avg_miss_latency(), Duration::ZERO);
        assert_eq!(metrics.avg_put_latency(), Duration::ZERO);
        assert_eq!(metrics.compression_ratio(), 1.0);
        assert_eq!(metrics.hit_rate(), 0.0);
        assert!(metrics.top_access_patterns(10).is_empty());
    }

    #[test]
    fn test_metrics_snapshot() {
        let metrics = CacheMetrics::new();

        // Set up some data
        metrics.record_hit(Duration::from_millis(10));
        metrics.record_miss(Duration::from_millis(20));
        metrics.record_put(Duration::from_millis(30));
        metrics.record_delete();
        metrics.record_eviction();
        metrics.record_error();
        metrics.update_size(100);
        metrics.update_compression_ratio(0.8);

        let snapshot = metrics.snapshot();

        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.misses, 1);
        assert_eq!(snapshot.puts, 1);
        assert_eq!(snapshot.deletes, 1);
        assert_eq!(snapshot.evictions, 1);
        assert_eq!(snapshot.errors, 1);
        assert_eq!(snapshot.hit_rate, 0.5);
        assert_eq!(snapshot.current_size, 100);
        assert_eq!(snapshot.max_size, 100);
        assert_eq!(snapshot.avg_hit_latency, Duration::from_millis(10));
        assert_eq!(snapshot.avg_miss_latency, Duration::from_millis(20));
        assert_eq!(snapshot.avg_put_latency, Duration::from_millis(30));
        assert_eq!(snapshot.compression_ratio, 0.8);
        assert!(snapshot.uptime > Duration::ZERO);
    }

    #[test]
    fn test_concurrent_operations() {
        let metrics = Arc::new(CacheMetrics::new());
        let num_threads = 10;
        let operations_per_thread = 100;

        let mut handles = Vec::new();

        // Spawn threads to perform concurrent operations
        for i in 0..num_threads {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for j in 0..operations_per_thread {
                    let latency = Duration::from_nanos((i * operations_per_thread + j) as u64);

                    match j % 4 {
                        0 => metrics_clone.record_hit(latency),
                        1 => metrics_clone.record_miss(latency),
                        2 => metrics_clone.record_put(latency),
                        3 => metrics_clone.record_delete(),
                        _ => unreachable!(),
                    }

                    if j % 10 == 0 {
                        metrics_clone.record_eviction();
                    }

                    if j % 20 == 0 {
                        metrics_clone.record_error();
                    }

                    metrics_clone.update_size(j);
                    metrics_clone.record_access_pattern(format!("pattern_{}", i));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify expected totals
        let expected_hits = num_threads * operations_per_thread / 4;
        let expected_misses = num_threads * operations_per_thread / 4;
        let expected_puts = num_threads * operations_per_thread / 4;
        let expected_deletes = num_threads * operations_per_thread / 4;

        // Calculate expected evictions: j % 10 == 0 means operations 0, 10, 20, ..., 90
        // For operations_per_thread = 100, that's 10 evictions per thread
        let expected_evictions = num_threads * operations_per_thread.div_ceil(10);
        let expected_errors = num_threads * operations_per_thread.div_ceil(20);

        assert_eq!(
            metrics.inner.hits.load(Ordering::Relaxed),
            expected_hits as u64
        );
        assert_eq!(
            metrics.inner.misses.load(Ordering::Relaxed),
            expected_misses as u64
        );
        assert_eq!(
            metrics.inner.puts.load(Ordering::Relaxed),
            expected_puts as u64
        );
        assert_eq!(
            metrics.inner.deletes.load(Ordering::Relaxed),
            expected_deletes as u64
        );
        assert_eq!(metrics.evictions(), expected_evictions as u64);
        assert_eq!(metrics.errors(), expected_errors as u64);

        // Verify hit rate is approximately 0.5 (hits / (hits + misses))
        let hit_rate = metrics.hit_rate();
        assert!((hit_rate - 0.5).abs() < 0.01);

        // Verify access patterns were recorded
        let patterns = metrics.top_access_patterns(num_threads);
        assert_eq!(patterns.len(), num_threads);

        for pattern in patterns {
            assert_eq!(pattern.1, operations_per_thread as u64);
        }
    }

    #[test]
    fn test_large_numbers() {
        let metrics = CacheMetrics::new();

        // Test with large numbers to ensure no overflow
        let large_latency = Duration::from_nanos(u64::MAX / 1000);

        for _ in 0..10 {
            metrics.record_hit(large_latency);
        }

        // Should not panic or overflow
        let avg_latency = metrics.avg_hit_latency();
        assert_eq!(avg_latency, large_latency);

        metrics.update_size(usize::MAX / 2);
        assert_eq!(metrics.current_size(), usize::MAX / 2);
    }

    #[test]
    fn test_zero_latency() {
        let metrics = CacheMetrics::new();

        metrics.record_hit(Duration::ZERO);
        metrics.record_miss(Duration::ZERO);
        metrics.record_put(Duration::ZERO);

        assert_eq!(metrics.avg_hit_latency(), Duration::ZERO);
        assert_eq!(metrics.avg_miss_latency(), Duration::ZERO);
        assert_eq!(metrics.avg_put_latency(), Duration::ZERO);
    }

    #[test]
    fn test_compression_ratio_edge_cases() {
        let metrics = CacheMetrics::new();

        // Test very small compression ratio
        metrics.update_compression_ratio(0.001);
        assert_eq!(metrics.compression_ratio(), 0.001);

        // Test very large compression ratio
        metrics.update_compression_ratio(1000.0);
        assert_eq!(metrics.compression_ratio(), 1000.0);

        // Test negative compression ratio (unusual but should be handled)
        metrics.update_compression_ratio(-1.0);
        assert_eq!(metrics.compression_ratio(), -1.0);
    }

    #[test]
    fn test_access_pattern_empty_string() {
        let metrics = CacheMetrics::new();

        metrics.record_access_pattern("".to_string());
        metrics.record_access_pattern("normal_pattern".to_string());
        metrics.record_access_pattern("".to_string());

        let patterns = metrics.top_access_patterns(10);
        assert_eq!(patterns.len(), 2);

        let empty_pattern_count = patterns.iter().find(|(p, _)| p.is_empty()).map(|(_, c)| *c);
        assert_eq!(empty_pattern_count, Some(2));
    }

    #[test]
    fn test_max_size_concurrent_updates() {
        let metrics = Arc::new(CacheMetrics::new());
        let num_threads = 10;

        let mut handles = Vec::new();

        for i in 0..num_threads {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                // Each thread updates size with different values
                for j in 0..100 {
                    let size = i * 100 + j;
                    metrics_clone.update_size(size);
                    thread::sleep(Duration::from_nanos(1)); // Small delay to increase contention
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Max size should be the largest value set by any thread
        let expected_max = (num_threads - 1) * 100 + 99;
        assert_eq!(metrics.max_size(), expected_max);
    }

    #[test]
    fn test_metrics_cloning() {
        let metrics1 = CacheMetrics::new();

        // Set up some data
        metrics1.record_hit(Duration::from_millis(10));
        metrics1.update_size(100);

        let metrics2 = metrics1.clone();

        // Both should show the same data
        assert_eq!(metrics1.inner.hits.load(Ordering::Relaxed), 1);
        assert_eq!(metrics2.inner.hits.load(Ordering::Relaxed), 1);

        // Modifications to one should affect the other (shared Arc)
        metrics1.record_hit(Duration::from_millis(20));
        assert_eq!(metrics2.inner.hits.load(Ordering::Relaxed), 2);

        metrics2.record_miss(Duration::from_millis(30));
        assert_eq!(metrics1.inner.misses.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_metrics_debug_format() {
        let metrics = CacheMetrics::new();
        metrics.record_hit(Duration::from_millis(10));

        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("CacheMetrics"));

        let snapshot = metrics.snapshot();
        let snapshot_debug = format!("{:?}", snapshot);
        assert!(snapshot_debug.contains("MetricsSnapshot"));
        assert!(snapshot_debug.contains("hits: 1"));
    }

    // Property-based tests using proptest
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_hit_rate_always_between_0_and_1(
            hits in 0u64..1000,
            misses in 0u64..1000
        ) {
            let metrics = CacheMetrics::new();

            for _ in 0..hits {
                metrics.record_hit(Duration::from_nanos(1));
            }

            for _ in 0..misses {
                metrics.record_miss(Duration::from_nanos(1));
            }

            let hit_rate = metrics.hit_rate();
            prop_assert!((0.0..=1.0).contains(&hit_rate));

            if hits + misses > 0 {
                let expected_rate = hits as f64 / (hits + misses) as f64;
                prop_assert!((hit_rate - expected_rate).abs() < f64::EPSILON);
            } else {
                prop_assert_eq!(hit_rate, 0.0);
            }
        }

        #[test]
        fn proptest_size_updates_maintain_max_correctly(
            sizes in prop::collection::vec(0usize..10000, 1..100)
        ) {
            let metrics = CacheMetrics::new();
            let mut expected_max = 0;

            for &size in &sizes {
                metrics.update_size(size);
                expected_max = expected_max.max(size);

                prop_assert_eq!(metrics.current_size(), size);
                prop_assert_eq!(metrics.max_size(), expected_max);
            }
        }

        #[test]
        fn proptest_latency_calculations_are_correct(
            latencies_ns in prop::collection::vec(1u64..1_000_000_000, 1..100)
        ) {
            let metrics = CacheMetrics::new();
            let mut total_ns = 0u64;

            for &latency_ns in &latencies_ns {
                let duration = Duration::from_nanos(latency_ns);
                metrics.record_hit(duration);
                total_ns += latency_ns;
            }

            let expected_avg = Duration::from_nanos(total_ns / latencies_ns.len() as u64);
            let actual_avg = metrics.avg_hit_latency();

            // Allow small rounding errors
            prop_assert!((actual_avg.as_nanos() as i128 - expected_avg.as_nanos() as i128).abs() <= 1);
        }

        #[test]
        fn proptest_operations_count_correctly(
            hits in 0u64..100,
            misses in 0u64..100,
            puts in 0u64..100,
            deletes in 0u64..100,
            evictions in 0u64..100,
            errors in 0u64..100
        ) {
            let metrics = CacheMetrics::new();

            for _ in 0..hits {
                metrics.record_hit(Duration::from_nanos(1));
            }
            for _ in 0..misses {
                metrics.record_miss(Duration::from_nanos(1));
            }
            for _ in 0..puts {
                metrics.record_put(Duration::from_nanos(1));
            }
            for _ in 0..deletes {
                metrics.record_delete();
            }
            for _ in 0..evictions {
                metrics.record_eviction();
            }
            for _ in 0..errors {
                metrics.record_error();
            }

            prop_assert_eq!(metrics.total_operations(), hits + misses + puts + deletes);
            prop_assert_eq!(metrics.evictions(), evictions);
            prop_assert_eq!(metrics.errors(), errors);
        }

        #[test]
        fn proptest_compression_ratio_preserves_values(
            ratio in -1000.0f64..1000.0
        ) {
            let metrics = CacheMetrics::new();
            metrics.update_compression_ratio(ratio);

            let retrieved_ratio = metrics.compression_ratio();
            prop_assert!((retrieved_ratio - ratio).abs() < f64::EPSILON);
        }

        #[test]
        fn proptest_access_patterns_counted_correctly(
            patterns in prop::collection::vec("[a-z]{1,10}", 1..50)
        ) {
            let metrics = CacheMetrics::new();
            let mut pattern_counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

            for pattern in &patterns {
                metrics.record_access_pattern(pattern.clone());
                *pattern_counts.entry(pattern.clone()).or_insert(0) += 1;
            }

            let top_patterns = metrics.top_access_patterns(pattern_counts.len());
            prop_assert_eq!(top_patterns.len(), pattern_counts.len());

            // Verify all patterns are present with correct counts
            for (pattern, expected_count) in &pattern_counts {
                let actual_count = top_patterns.iter()
                    .find(|(p, _)| p == pattern)
                    .map(|(_, c)| *c);
                prop_assert_eq!(actual_count, Some(*expected_count));
            }

            // Verify patterns are sorted by count (descending)
            for i in 1..top_patterns.len() {
                prop_assert!(top_patterns[i-1].1 >= top_patterns[i].1);
            }
        }

        #[test]
        fn proptest_reset_clears_all_metrics(
            operations in 1u64..100,
            size in 1usize..10000,
            ratio in 0.1f64..10.0
        ) {
            let metrics = CacheMetrics::new();

            // Set up some data
            for _ in 0..operations {
                metrics.record_hit(Duration::from_millis(10));
                metrics.record_miss(Duration::from_millis(20));
                metrics.record_put(Duration::from_millis(5));
                metrics.record_delete();
                metrics.record_eviction();
                metrics.record_error();
            }
            metrics.update_size(size);
            metrics.update_compression_ratio(ratio);
            metrics.record_access_pattern("test_pattern".to_string());

            // Verify data is set
            prop_assert!(metrics.total_operations() > 0);
            prop_assert!(metrics.evictions() > 0);
            prop_assert!(metrics.errors() > 0);
            prop_assert!(metrics.current_size() > 0);
            prop_assert!(metrics.max_size() > 0);
            prop_assert_ne!(metrics.compression_ratio(), 1.0);
            prop_assert!(!metrics.top_access_patterns(10).is_empty());

            // Reset and verify everything is cleared
            metrics.reset();

            prop_assert_eq!(metrics.total_operations(), 0);
            prop_assert_eq!(metrics.evictions(), 0);
            prop_assert_eq!(metrics.errors(), 0);
            prop_assert_eq!(metrics.current_size(), 0);
            prop_assert_eq!(metrics.max_size(), 0);
            prop_assert_eq!(metrics.avg_hit_latency(), Duration::ZERO);
            prop_assert_eq!(metrics.avg_miss_latency(), Duration::ZERO);
            prop_assert_eq!(metrics.avg_put_latency(), Duration::ZERO);
            prop_assert_eq!(metrics.compression_ratio(), 1.0);
            prop_assert_eq!(metrics.hit_rate(), 0.0);
            prop_assert!(metrics.top_access_patterns(10).is_empty());
        }

        #[test]
        fn proptest_snapshot_consistency(
            hits in 0u64..50,
            misses in 0u64..50,
            puts in 0u64..50
        ) {
            let metrics = CacheMetrics::new();

            for _ in 0..hits {
                metrics.record_hit(Duration::from_millis(10));
            }
            for _ in 0..misses {
                metrics.record_miss(Duration::from_millis(20));
            }
            for _ in 0..puts {
                metrics.record_put(Duration::from_millis(15));
            }

            let snapshot = metrics.snapshot();

            // Snapshot should reflect current state
            prop_assert_eq!(snapshot.hits, hits);
            prop_assert_eq!(snapshot.misses, misses);
            prop_assert_eq!(snapshot.puts, puts);
            prop_assert_eq!(snapshot.hit_rate, metrics.hit_rate());
            prop_assert_eq!(snapshot.avg_hit_latency, metrics.avg_hit_latency());
            prop_assert_eq!(snapshot.avg_miss_latency, metrics.avg_miss_latency());
            prop_assert_eq!(snapshot.avg_put_latency, metrics.avg_put_latency());
            prop_assert_eq!(snapshot.compression_ratio, metrics.compression_ratio());
        }

        #[test]
        fn proptest_concurrent_size_updates_maintain_atomicity(
            sizes in prop::collection::vec(0usize..1000, 10..50)
        ) {
            use std::sync::Arc;
            use std::thread;

            let metrics = Arc::new(CacheMetrics::new());
            let mut handles = Vec::new();

            for &size in &sizes {
                let metrics_clone = Arc::clone(&metrics);
                let handle = thread::spawn(move || {
                    metrics_clone.update_size(size);
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            let final_max = metrics.max_size();
            let expected_max = sizes.iter().max().copied().unwrap_or(0);

            prop_assert_eq!(final_max, expected_max);
            prop_assert!(sizes.contains(&metrics.current_size()));
        }
    }
}
