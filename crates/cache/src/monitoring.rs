//! Monitoring and observability for the cache system
//!
//! This module provides comprehensive monitoring capabilities including:
//! - Prometheus metrics export
//! - OpenTelemetry distributed tracing
//! - Cache hit rate analysis
//! - Performance flamegraphs
//! - Real-time dashboards

use crate::errors::{CacheError, RecoveryHint, Result};
pub use crate::traits::CacheStatistics;
use parking_lot::RwLock;
use prometheus::{CounterVec, Encoder, HistogramVec, IntGaugeVec, Registry, TextEncoder};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, Span};

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

/// Prometheus metrics for cache operations
struct MetricsCollector {
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

/// Performance profiler for flamegraph generation
struct PerformanceProfiler {
    /// Profiling data by operation
    profiles: RwLock<HashMap<String, ProfileData>>,
    /// Whether profiling is enabled
    enabled: AtomicU64,
    /// Sampling rate (1 in N operations)
    sampling_rate: u64,
}

struct ProfileData {
    samples: Vec<ProfileSample>,
    total_time: Duration,
    operation_count: u64,
}

#[allow(dead_code)]
struct ProfileSample {
    operation: String,
    duration: Duration,
    stack_trace: Vec<String>,
    timestamp: Instant,
}

/// Hit rate analyzer for cache effectiveness
struct HitRateAnalyzer {
    /// Hit rates by time window
    time_windows: RwLock<TimeWindowStats>,
    /// Hit rates by key pattern
    key_patterns: RwLock<HashMap<String, HitRateStats>>,
    /// Hit rates by operation type
    operation_types: RwLock<HashMap<String, HitRateStats>>,
}

struct TimeWindowStats {
    /// 1-minute window
    one_minute: RollingWindow,
    /// 5-minute window
    five_minutes: RollingWindow,
    /// 1-hour window
    one_hour: RollingWindow,
    /// 24-hour window
    one_day: RollingWindow,
}

struct RollingWindow {
    hits: AtomicU64,
    misses: AtomicU64,
    window_start: RwLock<Instant>,
    window_duration: Duration,
}

struct HitRateStats {
    hits: AtomicU64,
    misses: AtomicU64,
    last_access: RwLock<Instant>,
}

/// Real-time statistics collector
#[allow(dead_code)]
struct RealTimeStats {
    /// Current operations in flight
    operations_in_flight: AtomicU64,
    /// Peak operations per second
    peak_ops_per_second: AtomicU64,
    /// Average response time (microseconds)
    avg_response_time_us: AtomicU64,
    /// P99 response time (microseconds)
    p99_response_time_us: AtomicU64,
    /// Response time samples
    response_times: RwLock<Vec<u64>>,
}

impl CacheMonitor {
    /// Create a new cache monitor with full observability stack
    pub fn new(_service_name: &str) -> Result<Self> {
        // Initialize Prometheus registry
        let registry = Registry::new();

        // Initialize metrics
        let metrics = match Self::init_metrics(&registry) {
            Ok(m) => m,
            Err(e) => {
                return Err(CacheError::Configuration {
                    message: format!("Failed to initialize metrics: {e}"),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check Prometheus metrics configuration".to_string(),
                    },
                });
            }
        };

        // Initialize profiler
        let profiler = PerformanceProfiler {
            profiles: RwLock::new(HashMap::new()),
            enabled: AtomicU64::new(0),
            sampling_rate: 100, // Sample 1 in 100 operations
        };

        // Initialize hit rate analyzer
        let hit_rate_analyzer = HitRateAnalyzer {
            time_windows: RwLock::new(TimeWindowStats {
                one_minute: RollingWindow::new(Duration::from_secs(60)),
                five_minutes: RollingWindow::new(Duration::from_secs(300)),
                one_hour: RollingWindow::new(Duration::from_secs(3600)),
                one_day: RollingWindow::new(Duration::from_secs(86400)),
            }),
            key_patterns: RwLock::new(HashMap::new()),
            operation_types: RwLock::new(HashMap::new()),
        };

        // Initialize real-time stats
        let real_time_stats = RealTimeStats {
            operations_in_flight: AtomicU64::new(0),
            peak_ops_per_second: AtomicU64::new(0),
            avg_response_time_us: AtomicU64::new(0),
            p99_response_time_us: AtomicU64::new(0),
            response_times: RwLock::new(Vec::with_capacity(10000)),
        };

        let inner = Arc::new(MonitorInner {
            metrics,
            profiler,
            hit_rate_analyzer,
            real_time_stats,
            registry,
        });

        Ok(Self { inner })
    }

    /// Initialize Prometheus metrics
    fn init_metrics(
        registry: &Registry,
    ) -> std::result::Result<MetricsCollector, Box<dyn std::error::Error>> {
        use prometheus::{CounterVec, HistogramVec, IntGaugeVec, Opts};

        // Create specific metrics that tests expect
        let cache_operations = CounterVec::new(
            Opts::new(
                "cuenv_cache_operations_total",
                "Total number of cache operations",
            ),
            &["operation", "result"],
        )?;
        registry.register(Box::new(cache_operations.clone()))?;

        // Individual operation metrics for backward compatibility
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

        Ok(MetricsCollector {
            cache_operations,
            hits_counter,
            misses_counter,
            writes_counter,
            errors_counter,
            operation_duration,
            cache_gauges,
        })
    }

    /// Record a cache hit
    pub fn record_hit(&self, key: &str, operation: &str, duration: Duration) {
        // Update metrics
        self.inner
            .metrics
            .cache_operations
            .with_label_values(&[operation, "hit"])
            .inc();

        // Update specific hit counter
        let key_pattern = self.extract_key_pattern(key);
        self.inner
            .metrics
            .hits_counter
            .with_label_values(&[&key_pattern])
            .inc();

        self.inner
            .metrics
            .operation_duration
            .with_label_values(&[operation, "hit"])
            .observe(duration.as_secs_f64());

        // Update hit rate analyzer
        self.inner.hit_rate_analyzer.record_hit(key, operation);

        // Update real-time stats
        self.inner.real_time_stats.record_operation(duration);

        // Profile if enabled
        if self.inner.profiler.should_profile() {
            self.inner
                .profiler
                .record_operation(operation, duration, true);
        }
    }

    /// Record a cache miss
    pub fn record_miss(&self, key: &str, operation: &str, duration: Duration) {
        // Update metrics
        self.inner
            .metrics
            .cache_operations
            .with_label_values(&[operation, "miss"])
            .inc();

        // Update specific miss counter
        let key_pattern = self.extract_key_pattern(key);
        self.inner
            .metrics
            .misses_counter
            .with_label_values(&[&key_pattern])
            .inc();

        self.inner
            .metrics
            .operation_duration
            .with_label_values(&[operation, "miss"])
            .observe(duration.as_secs_f64());

        // Update hit rate analyzer
        self.inner.hit_rate_analyzer.record_miss(key, operation);

        // Update real-time stats
        self.inner.real_time_stats.record_operation(duration);

        // Profile if enabled
        if self.inner.profiler.should_profile() {
            self.inner
                .profiler
                .record_operation(operation, duration, false);
        }
    }

    /// Record a cache write
    pub fn record_write(&self, key: &str, _size_bytes: u64, duration: Duration) {
        self.inner
            .metrics
            .cache_operations
            .with_label_values(&["write", "success"])
            .inc();

        // Update specific write counter
        let key_pattern = self.extract_key_pattern(key);
        self.inner
            .metrics
            .writes_counter
            .with_label_values(&[&key_pattern])
            .inc();

        self.inner
            .metrics
            .operation_duration
            .with_label_values(&["write", "success"])
            .observe(duration.as_secs_f64());

        self.inner.real_time_stats.record_operation(duration);
    }

    /// Record a cache removal
    pub fn record_removal(&self, _key: &str, duration: Duration) {
        self.inner
            .metrics
            .cache_operations
            .with_label_values(&["remove", "success"])
            .inc();

        self.inner
            .metrics
            .operation_duration
            .with_label_values(&["remove", "success"])
            .observe(duration.as_secs_f64());

        self.inner.real_time_stats.record_operation(duration);
    }

    /// Record a cache error
    pub fn record_error(&self, operation: &str, _error: &CacheError) {
        self.inner
            .metrics
            .cache_operations
            .with_label_values(&[operation, "error"])
            .inc();

        // Update specific error counter
        self.inner
            .metrics
            .errors_counter
            .with_label_values(&[operation])
            .inc();
    }

    /// Record a cache eviction
    pub fn record_eviction(&self, reason: &str, count: u64) {
        for _ in 0..count {
            self.inner
                .metrics
                .cache_operations
                .with_label_values(&["eviction", reason])
                .inc();
        }
    }

    /// Update observable gauges with current statistics
    pub fn update_statistics(&self, stats: &CacheStatistics, memory_bytes: u64, disk_bytes: u64) {
        // Update gauge metrics
        self.inner
            .metrics
            .cache_gauges
            .with_label_values(&["entries"])
            .set(stats.entry_count as i64);

        self.inner
            .metrics
            .cache_gauges
            .with_label_values(&["size_bytes"])
            .set(stats.total_bytes as i64);

        self.inner
            .metrics
            .cache_gauges
            .with_label_values(&["memory_bytes"])
            .set(memory_bytes as i64);

        self.inner
            .metrics
            .cache_gauges
            .with_label_values(&["disk_bytes"])
            .set(disk_bytes as i64);

        // Calculate and update hit rate
        let total = stats.hits + stats.misses;
        if total > 0 {
            let hit_rate = (stats.hits as f64 / total as f64 * 100.0) as i64;
            self.inner
                .metrics
                .cache_gauges
                .with_label_values(&["hit_rate_percent"])
                .set(hit_rate);
        }

        debug!(
            "Cache statistics - hits: {}, misses: {}, size: {} bytes, entries: {}",
            stats.hits, stats.misses, stats.total_bytes, stats.entry_count
        );
    }

    /// Start a traced operation
    pub fn start_operation(&self, operation: &str, key: &str) -> TracedOperation {
        let span = tracing::info_span!("cache_operation", operation = operation, key = key);

        self.inner
            .real_time_stats
            .operations_in_flight
            .fetch_add(1, Ordering::Relaxed);

        TracedOperation {
            span,
            start_time: Instant::now(),
            monitor: self.clone(),
            completed: std::sync::atomic::AtomicBool::new(false),
        }
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
        self.inner.profiler.enabled.store(1, Ordering::Relaxed);
        info!("Performance profiling enabled");
    }

    /// Disable performance profiling
    pub fn disable_profiling(&self) {
        self.inner.profiler.enabled.store(0, Ordering::Relaxed);
        info!("Performance profiling disabled");
    }

    /// Generate flamegraph data
    pub fn generate_flamegraph(&self) -> String {
        self.inner.profiler.generate_flamegraph()
    }

    /// Get real-time performance statistics
    pub fn real_time_stats(&self) -> RealTimeStatsReport {
        self.inner.real_time_stats.generate_report()
    }

    /// Size bucket for metrics
    #[allow(dead_code)]
    fn size_bucket(size_bytes: u64) -> &'static str {
        match size_bytes {
            0..=1024 => "small",
            1025..=65536 => "medium",
            65537..=1048576 => "large",
            _ => "xlarge",
        }
    }

    /// Extract a pattern from a cache key for grouping metrics
    fn extract_key_pattern(&self, key: &str) -> String {
        // Simple pattern extraction - group by prefix before first colon or slash
        if let Some(pos) = key.find(':').or_else(|| key.find('/')) {
            key[..pos].to_string()
        } else {
            "other".to_string()
        }
    }
}

impl Clone for CacheMonitor {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Traced operation handle
pub struct TracedOperation {
    span: Span,
    start_time: Instant,
    monitor: CacheMonitor,
    completed: std::sync::atomic::AtomicBool,
}

impl TracedOperation {
    /// Complete the operation successfully
    pub fn complete(self) {
        if !self.completed.swap(true, Ordering::Relaxed) {
            tracing::info!(parent: &self.span, "Operation completed successfully");

            let _duration = self.start_time.elapsed();
            self.monitor
                .inner
                .real_time_stats
                .operations_in_flight
                .fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Complete the operation with an error
    pub fn error(self, error: &CacheError) {
        if !self.completed.swap(true, Ordering::Relaxed) {
            tracing::error!(parent: &self.span, error = %error, "Operation failed");

            self.monitor
                .inner
                .real_time_stats
                .operations_in_flight
                .fetch_sub(1, Ordering::Relaxed);
        }
    }
}

impl Drop for TracedOperation {
    fn drop(&mut self) {
        // Ensure we decrement the counter even if the operation wasn't properly completed
        if !self.completed.swap(true, Ordering::Relaxed) {
            self.monitor
                .inner
                .real_time_stats
                .operations_in_flight
                .fetch_sub(1, Ordering::Relaxed);
        }
    }
}

impl RollingWindow {
    fn new(duration: Duration) -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            window_start: RwLock::new(Instant::now()),
            window_duration: duration,
        }
    }

    fn record_hit(&self) {
        self.roll_window();
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn record_miss(&self) {
        self.roll_window();
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    fn hit_rate(&self) -> f64 {
        self.roll_window();
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    fn roll_window(&self) {
        let now = Instant::now();
        let mut window_start = self.window_start.write();

        if now.duration_since(*window_start) > self.window_duration {
            // Reset the window
            self.hits.store(0, Ordering::Relaxed);
            self.misses.store(0, Ordering::Relaxed);
            *window_start = now;
        }
    }
}

impl HitRateAnalyzer {
    fn record_hit(&self, key: &str, operation: &str) {
        // Update time windows
        {
            let windows = self.time_windows.read();
            windows.one_minute.record_hit();
            windows.five_minutes.record_hit();
            windows.one_hour.record_hit();
            windows.one_day.record_hit();
        }

        // Update key pattern stats
        if let Some(pattern) = Self::extract_pattern(key) {
            let mut patterns = self.key_patterns.write();
            let stats = patterns.entry(pattern).or_insert_with(|| HitRateStats {
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
                last_access: RwLock::new(Instant::now()),
            });
            stats.hits.fetch_add(1, Ordering::Relaxed);
            *stats.last_access.write() = Instant::now();
        }

        // Update operation type stats
        {
            let mut op_types = self.operation_types.write();
            let stats = op_types
                .entry(operation.to_string())
                .or_insert_with(|| HitRateStats {
                    hits: AtomicU64::new(0),
                    misses: AtomicU64::new(0),
                    last_access: RwLock::new(Instant::now()),
                });
            stats.hits.fetch_add(1, Ordering::Relaxed);
            *stats.last_access.write() = Instant::now();
        }
    }

    fn record_miss(&self, key: &str, operation: &str) {
        // Update time windows
        {
            let windows = self.time_windows.read();
            windows.one_minute.record_miss();
            windows.five_minutes.record_miss();
            windows.one_hour.record_miss();
            windows.one_day.record_miss();
        }

        // Update key pattern stats
        if let Some(pattern) = Self::extract_pattern(key) {
            let mut patterns = self.key_patterns.write();
            let stats = patterns.entry(pattern).or_insert_with(|| HitRateStats {
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
                last_access: RwLock::new(Instant::now()),
            });
            stats.misses.fetch_add(1, Ordering::Relaxed);
            *stats.last_access.write() = Instant::now();
        }

        // Update operation type stats
        {
            let mut op_types = self.operation_types.write();
            let stats = op_types
                .entry(operation.to_string())
                .or_insert_with(|| HitRateStats {
                    hits: AtomicU64::new(0),
                    misses: AtomicU64::new(0),
                    last_access: RwLock::new(Instant::now()),
                });
            stats.misses.fetch_add(1, Ordering::Relaxed);
            *stats.last_access.write() = Instant::now();
        }
    }

    fn overall_hit_rate(&self) -> f64 {
        let windows = self.time_windows.read();
        windows.one_hour.hit_rate()
    }

    fn generate_report(&self) -> HitRateReport {
        let windows = self.time_windows.read();

        let mut key_patterns = Vec::new();
        for (pattern, stats) in self.key_patterns.read().iter() {
            let hits = stats.hits.load(Ordering::Relaxed);
            let misses = stats.misses.load(Ordering::Relaxed);
            let total = hits + misses;
            if total > 0 {
                key_patterns.push(PatternStats {
                    pattern: pattern.clone(),
                    hit_rate: hits as f64 / total as f64,
                    total_accesses: total,
                });
            }
        }
        key_patterns.sort_by(|a, b| b.total_accesses.cmp(&a.total_accesses));

        let mut operation_types = Vec::new();
        for (op_type, stats) in self.operation_types.read().iter() {
            let hits = stats.hits.load(Ordering::Relaxed);
            let misses = stats.misses.load(Ordering::Relaxed);
            let total = hits + misses;
            if total > 0 {
                operation_types.push(OperationStats {
                    operation: op_type.clone(),
                    hit_rate: hits as f64 / total as f64,
                    total_calls: total,
                });
            }
        }
        operation_types.sort_by(|a, b| b.total_calls.cmp(&a.total_calls));

        HitRateReport {
            one_minute: windows.one_minute.hit_rate(),
            five_minutes: windows.five_minutes.hit_rate(),
            one_hour: windows.one_hour.hit_rate(),
            one_day: windows.one_day.hit_rate(),
            key_patterns,
            operation_types,
        }
    }

    fn extract_pattern(key: &str) -> Option<String> {
        // Extract pattern from key (e.g., "user:123" -> "user:*")
        if let Some(colon_pos) = key.find(':') {
            Some(format!("{}:*", &key[..colon_pos]))
        } else if let Some(slash_pos) = key.find('/') {
            Some(format!("{}/*", &key[..slash_pos]))
        } else {
            None
        }
    }
}

impl PerformanceProfiler {
    fn should_profile(&self) -> bool {
        if self.enabled.load(Ordering::Relaxed) == 0 {
            return false;
        }

        // Simple sampling
        fastrand::u64(1..=self.sampling_rate) == 1
    }

    fn record_operation(&self, operation: &str, duration: Duration, hit: bool) {
        let sample = ProfileSample {
            operation: format!("{operation}_{}", if hit { "hit" } else { "miss" }),
            duration,
            stack_trace: Self::capture_stack_trace(),
            timestamp: Instant::now(),
        };

        let mut profiles = self.profiles.write();
        let profile = profiles
            .entry(operation.to_string())
            .or_insert_with(|| ProfileData {
                samples: Vec::new(),
                total_time: Duration::ZERO,
                operation_count: 0,
            });

        profile.samples.push(sample);
        profile.total_time += duration;
        profile.operation_count += 1;

        // Keep only recent samples (last 10000)
        if profile.samples.len() > 10000 {
            profile.samples.drain(0..5000);
        }
    }

    fn capture_stack_trace() -> Vec<String> {
        // In a real implementation, this would capture the actual stack trace
        // For now, return a placeholder
        vec![
            "cuenv::cache::unified::get".to_string(),
            "cuenv::cache::streaming::read".to_string(),
            "tokio::runtime::Runtime::block_on".to_string(),
        ]
    }

    fn generate_flamegraph(&self) -> String {
        let profiles = self.profiles.read();
        let mut output = String::new();

        for (_operation, profile) in profiles.iter() {
            for sample in &profile.samples {
                // Format: stack;frames;here count
                let stack = sample.stack_trace.join(";");
                let count = sample.duration.as_micros();
                output.push_str(&format!("{};{} {}\n", stack, sample.operation, count));
            }
        }

        output
    }
}

impl RealTimeStats {
    fn record_operation(&self, duration: Duration) {
        let duration_us = duration.as_micros() as u64;

        // Update response times
        {
            let mut times = self.response_times.write();
            times.push(duration_us);

            // Keep only recent samples
            if times.len() > 10000 {
                times.drain(0..5000);
            }

            // Calculate statistics
            if !times.is_empty() {
                let sum: u64 = times.iter().sum();
                let avg = sum / times.len() as u64;
                self.avg_response_time_us.store(avg, Ordering::Relaxed);

                // Calculate P99
                let mut sorted = times.clone();
                sorted.sort_unstable();
                let p99_index = (sorted.len() as f64 * 0.99) as usize;
                let p99 = sorted.get(p99_index).copied().unwrap_or(0);
                self.p99_response_time_us.store(p99, Ordering::Relaxed);
            }
        }
    }

    fn generate_report(&self) -> RealTimeStatsReport {
        RealTimeStatsReport {
            operations_in_flight: self.operations_in_flight.load(Ordering::Relaxed),
            avg_response_time_us: self.avg_response_time_us.load(Ordering::Relaxed),
            p99_response_time_us: self.p99_response_time_us.load(Ordering::Relaxed),
        }
    }
}

/// Hit rate analysis report
#[derive(Debug, Clone)]
pub struct HitRateReport {
    pub one_minute: f64,
    pub five_minutes: f64,
    pub one_hour: f64,
    pub one_day: f64,
    pub key_patterns: Vec<PatternStats>,
    pub operation_types: Vec<OperationStats>,
}

#[derive(Debug, Clone)]
pub struct PatternStats {
    pub pattern: String,
    pub hit_rate: f64,
    pub total_accesses: u64,
}

#[derive(Debug, Clone)]
pub struct OperationStats {
    pub operation: String,
    pub hit_rate: f64,
    pub total_calls: u64,
}

/// Real-time statistics report
#[derive(Debug, Clone)]
pub struct RealTimeStatsReport {
    pub operations_in_flight: u64,
    pub avg_response_time_us: u64,
    pub p99_response_time_us: u64,
}

impl CacheError {
    /// Get error type for metrics
    #[allow(dead_code)]
    fn error_type(&self) -> &'static str {
        match self {
            CacheError::InvalidKey { .. } => "invalid_key",
            CacheError::Serialization { .. } => "serialization",
            CacheError::Corruption { .. } => "corruption",
            CacheError::Io { .. } => "io",
            CacheError::CapacityExceeded { .. } => "capacity_exceeded",
            CacheError::Configuration { .. } => "configuration",
            CacheError::StoreUnavailable { .. } => "store_unavailable",
            CacheError::ConcurrencyConflict { .. } => "concurrency_conflict",
            // RemoteError variant has been removed - map to network error
            CacheError::Network { .. } => "remote_error",
            CacheError::Timeout { .. } => "timeout",
            // Handle any other variants
            _ => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_window() {
        let window = RollingWindow::new(Duration::from_secs(1));

        // Record some hits and misses
        for _ in 0..7 {
            window.record_hit();
        }
        for _ in 0..3 {
            window.record_miss();
        }

        // Check hit rate
        assert!((window.hit_rate() - 0.7).abs() < 0.01);

        // Wait for window to expire
        std::thread::sleep(Duration::from_millis(1100));

        // After expiry, should reset
        assert_eq!(window.hit_rate(), 0.0);
    }

    #[test]
    fn test_pattern_extraction() {
        assert_eq!(
            HitRateAnalyzer::extract_pattern("user:123"),
            Some("user:*".to_string())
        );
        assert_eq!(
            HitRateAnalyzer::extract_pattern("path/to/file"),
            Some("path/*".to_string())
        );
        assert_eq!(HitRateAnalyzer::extract_pattern("simple_key"), None);
    }

    #[test]
    fn test_size_bucket() {
        assert_eq!(CacheMonitor::size_bucket(512), "small");
        assert_eq!(CacheMonitor::size_bucket(32768), "medium");
        assert_eq!(CacheMonitor::size_bucket(524288), "large");
        assert_eq!(CacheMonitor::size_bucket(2097152), "xlarge");
    }
}
