//! Main cache monitoring system

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::CacheStatistics;
use prometheus::{Encoder, Registry, TextEncoder};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

use super::analyzer::HitRateAnalyzer;
use super::metrics::MetricsCollector;
use super::profiler::PerformanceProfiler;
use super::stats::RealTimeStats;
use super::traced::TracedOperation;
use super::types::HitRateReport;

/// Cache monitoring system with comprehensive observability
pub struct CacheMonitor {
    inner: Arc<MonitorInner>,
}

struct MonitorInner {
    /// Prometheus metrics
    metrics: MetricsCollector,
    /// Performance profiler
    profiler: PerformanceProfiler,
    /// Hit rate analyzer
    hit_rate_analyzer: HitRateAnalyzer,
    /// Real-time statistics
    real_time_stats: RealTimeStats,
    /// Prometheus registry
    registry: Registry,
}

impl CacheMonitor {
    /// Create a new cache monitor with full observability stack
    pub fn new(_service_name: &str) -> Result<Self> {
        let registry = Registry::new();
        let metrics = MetricsCollector::init(&registry).map_err(|e| CacheError::Configuration {
            message: format!("Failed to initialize metrics: {e}"),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check Prometheus metrics configuration".to_string(),
            },
        })?;

        let profiler = PerformanceProfiler::new();
        let hit_rate_analyzer = HitRateAnalyzer::new();
        let real_time_stats = RealTimeStats::new();

        let inner = Arc::new(MonitorInner {
            metrics,
            profiler,
            hit_rate_analyzer,
            real_time_stats,
            registry,
        });

        Ok(Self { inner })
    }

    /// Record a cache hit
    pub fn record_hit(&self, key: &str, operation: &str, duration: Duration) {
        self.inner
            .metrics
            .record_hit(operation, &self.extract_key_pattern(key));
        self.inner
            .metrics
            .record_operation_duration(operation, "hit", duration);
        self.inner.hit_rate_analyzer.record_hit(key, operation);
        self.inner.real_time_stats.record_operation(duration);

        if self.inner.profiler.should_profile() {
            self.inner
                .profiler
                .record_operation(operation, duration, true);
        }
    }

    /// Record a cache miss
    pub fn record_miss(&self, key: &str, operation: &str, duration: Duration) {
        self.inner
            .metrics
            .record_miss(operation, &self.extract_key_pattern(key));
        self.inner
            .metrics
            .record_operation_duration(operation, "miss", duration);
        self.inner.hit_rate_analyzer.record_miss(key, operation);
        self.inner.real_time_stats.record_operation(duration);

        if self.inner.profiler.should_profile() {
            self.inner
                .profiler
                .record_operation(operation, duration, false);
        }
    }

    /// Record a cache write
    pub fn record_write(&self, key: &str, _size_bytes: u64, duration: Duration) {
        let key_pattern = self.extract_key_pattern(key);
        self.inner.metrics.record_write(&key_pattern);
        self.inner
            .metrics
            .record_operation_duration("write", "success", duration);
        self.inner.real_time_stats.record_operation(duration);
    }

    /// Record a cache removal
    pub fn record_removal(&self, _key: &str, duration: Duration) {
        self.inner.metrics.record_operation("remove", "success");
        self.inner
            .metrics
            .record_operation_duration("remove", "success", duration);
        self.inner.real_time_stats.record_operation(duration);
    }

    /// Record a cache error
    pub fn record_error(&self, operation: &str, _error: &CacheError) {
        self.inner.metrics.record_error(operation);
    }

    /// Record a cache eviction
    pub fn record_eviction(&self, reason: &str, count: u64) {
        self.inner.metrics.record_eviction(reason, count);
    }

    /// Update observable gauges with current statistics
    pub fn update_statistics(&self, stats: &CacheStatistics, memory_bytes: u64, disk_bytes: u64) {
        self.inner
            .metrics
            .update_gauges(stats, memory_bytes, disk_bytes);

        debug!(
            "Cache statistics - hits: {}, misses: {}, size: {} bytes, entries: {}",
            stats.hits, stats.misses, stats.total_bytes, stats.entry_count
        );
    }

    /// Start a traced operation
    pub fn start_operation(&self, operation: &str, key: &str) -> TracedOperation {
        self.inner.real_time_stats.increment_in_flight();
        TracedOperation::new(operation, key, self.clone())
    }

    /// Get Prometheus metrics as text
    pub fn metrics_text(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.inner.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }

    /// Get current hit rate
    pub fn hit_rate(&self) -> f64 {
        self.inner.hit_rate_analyzer.overall_hit_rate()
    }

    /// Get hit rate analysis report
    pub fn hit_rate_report(&self) -> HitRateReport {
        self.inner.hit_rate_analyzer.generate_report()
    }

    /// Enable performance profiling
    pub fn enable_profiling(&self) {
        self.inner.profiler.enable();
        info!("Performance profiling enabled");
    }

    /// Disable performance profiling
    pub fn disable_profiling(&self) {
        self.inner.profiler.disable();
        info!("Performance profiling disabled");
    }

    /// Generate flamegraph data
    pub fn generate_flamegraph(&self) -> String {
        self.inner.profiler.generate_flamegraph()
    }

    /// Get real-time performance statistics
    pub fn real_time_stats(&self) -> super::stats::RealTimeStatsReport {
        self.inner.real_time_stats.generate_report()
    }

    /// Extract a pattern from a cache key for grouping metrics
    fn extract_key_pattern(&self, key: &str) -> String {
        if let Some(pos) = key.find(':').or_else(|| key.find('/')) {
            key[..pos].to_string()
        } else {
            "other".to_string()
        }
    }

    pub(super) fn decrement_in_flight(&self) {
        self.inner.real_time_stats.decrement_in_flight();
    }
}

impl Clone for CacheMonitor {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
