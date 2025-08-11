//! Prometheus metrics collection for cache operations

use crate::traits::CacheStatistics;
use prometheus::{CounterVec, HistogramVec, IntGaugeVec, Opts, Registry};
use std::time::Duration;

/// Prometheus metrics for cache operations
pub struct MetricsCollector {
    /// Cache operation counter
    cache_operations: CounterVec,
    /// Cache hits counter
    hits_counter: CounterVec,
    /// Cache misses counter
    misses_counter: CounterVec,
    /// Cache writes counter
    writes_counter: CounterVec,
    /// Cache errors counter
    errors_counter: CounterVec,
    /// Operation duration histogram
    operation_duration: HistogramVec,
    /// Cache size gauges
    cache_gauges: IntGaugeVec,
}

impl MetricsCollector {
    /// Initialize Prometheus metrics
    pub fn init(registry: &Registry) -> Result<Self, Box<dyn std::error::Error>> {
        let cache_operations = CounterVec::new(
            Opts::new(
                "cuenv_cache_operations_total",
                "Total number of cache operations",
            ),
            &["operation", "result"],
        )?;
        registry.register(Box::new(cache_operations.clone()))?;

        let hits_counter = CounterVec::new(
            Opts::new("cuenv_cache_hits_total", "Total number of cache hits"),
            &["key_pattern"],
        )?;
        registry.register(Box::new(hits_counter.clone()))?;

        let misses_counter = CounterVec::new(
            Opts::new("cuenv_cache_misses_total", "Total number of cache misses"),
            &["key_pattern"],
        )?;
        registry.register(Box::new(misses_counter.clone()))?;

        let writes_counter = CounterVec::new(
            Opts::new("cuenv_cache_writes_total", "Total number of cache writes"),
            &["key_pattern"],
        )?;
        registry.register(Box::new(writes_counter.clone()))?;

        let errors_counter = CounterVec::new(
            Opts::new("cuenv_cache_errors_total", "Total number of cache errors"),
            &["error_type"],
        )?;
        registry.register(Box::new(errors_counter.clone()))?;

        let operation_duration = HistogramVec::new(
            Opts::new(
                "cuenv_cache_operation_duration_seconds",
                "Cache operation duration in seconds",
            )
            .into(),
            &["operation", "result"],
        )?;
        registry.register(Box::new(operation_duration.clone()))?;

        let cache_gauges = IntGaugeVec::new(
            Opts::new("cuenv_cache_stats", "Cache statistics"),
            &["metric"],
        )?;
        registry.register(Box::new(cache_gauges.clone()))?;

        Ok(Self {
            cache_operations,
            hits_counter,
            misses_counter,
            writes_counter,
            errors_counter,
            operation_duration,
            cache_gauges,
        })
    }

    pub fn record_hit(&self, operation: &str, key_pattern: &str) {
        self.cache_operations
            .with_label_values(&[operation, "hit"])
            .inc();
        self.hits_counter.with_label_values(&[key_pattern]).inc();
    }

    pub fn record_miss(&self, operation: &str, key_pattern: &str) {
        self.cache_operations
            .with_label_values(&[operation, "miss"])
            .inc();
        self.misses_counter.with_label_values(&[key_pattern]).inc();
    }

    pub fn record_write(&self, key_pattern: &str) {
        self.cache_operations
            .with_label_values(&["write", "success"])
            .inc();
        self.writes_counter.with_label_values(&[key_pattern]).inc();
    }

    pub fn record_operation(&self, operation: &str, result: &str) {
        self.cache_operations
            .with_label_values(&[operation, result])
            .inc();
    }

    pub fn record_error(&self, operation: &str) {
        self.cache_operations
            .with_label_values(&[operation, "error"])
            .inc();
        self.errors_counter.with_label_values(&[operation]).inc();
    }

    pub fn record_eviction(&self, reason: &str, count: u64) {
        for _ in 0..count {
            self.cache_operations
                .with_label_values(&["eviction", reason])
                .inc();
        }
    }

    pub fn record_operation_duration(&self, operation: &str, result: &str, duration: Duration) {
        self.operation_duration
            .with_label_values(&[operation, result])
            .observe(duration.as_secs_f64());
    }

    pub fn update_gauges(&self, stats: &CacheStatistics, memory_bytes: u64, disk_bytes: u64) {
        self.cache_gauges
            .with_label_values(&["entries"])
            .set(stats.entry_count as i64);

        self.cache_gauges
            .with_label_values(&["size_bytes"])
            .set(stats.total_bytes as i64);

        self.cache_gauges
            .with_label_values(&["memory_bytes"])
            .set(memory_bytes as i64);

        self.cache_gauges
            .with_label_values(&["disk_bytes"])
            .set(disk_bytes as i64);

        // Calculate and update hit rate
        let total = stats.hits + stats.misses;
        if total > 0 {
            let hit_rate = (stats.hits as f64 / total as f64 * 100.0) as i64;
            self.cache_gauges
                .with_label_values(&["hit_rate_percent"])
                .set(hit_rate);
        }
    }
}
