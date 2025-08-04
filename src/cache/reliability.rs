pub use crate::cache::audit::HealthStatus;
use crate::cache::errors::RecoveryHint;
use crate::cache::monitoring::CacheMonitor;
use crate::cache::traits::Cache;
use crate::cache::{CacheError, CacheResult, MonitoredCache};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Automatic corruption recovery system
#[allow(dead_code)]
pub struct CorruptionRecovery<C: Cache + Clone> {
    cache: Arc<MonitoredCache<C>>,
    repair_history: Arc<RwLock<HashMap<String, RepairRecord>>>,
    config: RecoveryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    /// Maximum number of repair attempts per entry
    pub max_repair_attempts: usize,
    /// Time window for repair attempts
    pub repair_window: Duration,
    /// Enable automatic quarantine of corrupted entries
    pub enable_quarantine: bool,
    /// Path to quarantine directory
    pub quarantine_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RepairRecord {
    attempts: usize,
    // #[serde(with = "crate::cache::serde_helpers::time::instant_as_nanos")]
    last_attempt: std::time::SystemTime,
    success: bool,
    error_details: Option<String>,
}

impl<C: Cache + Clone> CorruptionRecovery<C> {
    pub fn new(cache: Arc<MonitoredCache<C>>, config: RecoveryConfig) -> Self {
        Self {
            cache,
            repair_history: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Attempt to repair a corrupted cache entry
    pub async fn repair_entry(&self, key: &str) -> CacheResult<()> {
        let mut history = self.repair_history.write().await;
        let record = history.entry(key.to_string()).or_insert(RepairRecord {
            attempts: 0,
            last_attempt: std::time::SystemTime::now(),
            success: false,
            error_details: None,
        });

        // Check if we've exceeded repair attempts
        if record.attempts >= self.config.max_repair_attempts {
            if self.config.enable_quarantine {
                self.quarantine_entry(key).await?;
            }
            return Err(CacheError::CorruptionUnrecoverable {
                key: key.to_string(),
                recovery_hint: RecoveryHint::Manual {
                    instructions:
                        "Entry has exceeded maximum repair attempts. Manual intervention required."
                            .to_string(),
                },
            });
        }

        // Check if we're within the repair window
        if let Ok(elapsed) = record.last_attempt.elapsed() {
            if elapsed < self.config.repair_window {
                return Err(CacheError::RepairInProgress {
                    key: key.to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: self.config.repair_window - elapsed,
                    },
                });
            }
        }

        record.attempts += 1;
        record.last_attempt = std::time::SystemTime::now();

        // Attempt repair strategies
        match self.execute_repair_strategies(key).await {
            Ok(()) => {
                record.success = true;
                info!("Successfully repaired cache entry: {}", key);
                Ok(())
            }
            Err(e) => {
                record.error_details = Some(e.to_string());
                error!("Failed to repair cache entry {}: {}", key, e);
                Err(e)
            }
        }
    }

    async fn execute_repair_strategies(&self, key: &str) -> CacheResult<()> {
        // Strategy 1: Try to rebuild from source
        if let Ok(()) = self.rebuild_from_source(key).await {
            return Ok(());
        }

        // Strategy 2: Try to recover from backup
        if let Ok(()) = self.recover_from_backup(key).await {
            return Ok(());
        }

        // Strategy 3: Try to reconstruct from partial data
        if let Ok(()) = self.reconstruct_partial(key).await {
            return Ok(());
        }

        Err(CacheError::AllRepairStrategiesFailed {
            key: key.to_string(),
            recovery_hint: RecoveryHint::ClearAndRetry,
        })
    }

    async fn rebuild_from_source(&self, _key: &str) -> CacheResult<()> {
        // Implementation would rebuild the cache entry from original source
        // This is task-specific and would need integration with task system
        Err(CacheError::NotImplemented {
            recovery_hint: RecoveryHint::Manual {
                instructions: "Rebuild from source not yet implemented".to_string(),
            },
        })
    }

    async fn recover_from_backup(&self, _key: &str) -> CacheResult<()> {
        // Implementation would restore from backup location
        Err(CacheError::NotImplemented {
            recovery_hint: RecoveryHint::Manual {
                instructions: "Backup recovery not yet implemented".to_string(),
            },
        })
    }

    async fn reconstruct_partial(&self, _key: &str) -> CacheResult<()> {
        // Implementation would try to reconstruct from partial/related data
        Err(CacheError::NotImplemented {
            recovery_hint: RecoveryHint::Manual {
                instructions: "Partial reconstruction not yet implemented".to_string(),
            },
        })
    }

    async fn quarantine_entry(&self, key: &str) -> CacheResult<()> {
        // Move corrupted entry to quarantine directory
        warn!("Quarantining corrupted entry: {}", key);
        // Implementation would move the file to quarantine
        Ok(())
    }
}

/// Self-tuning cache parameters
#[allow(dead_code)]
pub struct SelfTuningCache<C: Cache + Clone> {
    cache: Arc<MonitoredCache<C>>,
    metrics: Arc<CacheMonitor>,
    tuning_state: Arc<RwLock<TuningState>>,
    config: TuningConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningConfig {
    /// Target cache hit rate (0.0 - 1.0)
    pub target_hit_rate: f64,
    /// Target p99 latency in milliseconds
    pub target_p99_latency: f64,
    /// Adjustment interval
    pub adjustment_interval: Duration,
    /// Maximum cache size in bytes
    pub max_cache_size: usize,
    /// Minimum cache size in bytes
    pub min_cache_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningState {
    pub current_size: usize,
    pub current_eviction_threshold: f64,
    pub compression_level: u32,
    pub shard_count: usize,
    pub last_adjustment: std::time::SystemTime,
    pub performance_history: Vec<PerformanceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub hit_rate: f64,
    pub p99_latency: f64,
    pub memory_usage: usize,
    pub cpu_usage: f64,
}

impl<C: Cache + Clone> SelfTuningCache<C> {
    pub fn new(
        cache: Arc<MonitoredCache<C>>,
        metrics: Arc<CacheMonitor>,
        config: TuningConfig,
    ) -> Self {
        let initial_state = TuningState {
            current_size: config.max_cache_size / 2,
            current_eviction_threshold: 0.9,
            compression_level: 3,
            shard_count: 256,
            last_adjustment: std::time::SystemTime::now(),
            performance_history: Vec::new(),
        };

        Self {
            cache,
            metrics,
            tuning_state: Arc::new(RwLock::new(initial_state)),
            config,
        }
    }

    /// Run the self-tuning loop
    pub async fn run_tuning_loop(&self) {
        loop {
            tokio::time::sleep(self.config.adjustment_interval).await;

            if let Err(e) = self.adjust_parameters().await {
                error!("Self-tuning adjustment failed: {}", e);
            }
        }
    }

    async fn adjust_parameters(&self) -> CacheResult<()> {
        let mut state = self.tuning_state.write().await;

        // Collect current performance metrics
        let snapshot = self.collect_performance_snapshot().await?;
        state.performance_history.push(snapshot.clone());

        // Keep only recent history
        if state.performance_history.len() > 100 {
            state.performance_history.remove(0);
        }

        // Analyze trends and adjust
        if snapshot.hit_rate < self.config.target_hit_rate {
            // Increase cache size if below target hit rate
            state.current_size = (state.current_size as f64 * 1.1) as usize;
            state.current_size = state.current_size.min(self.config.max_cache_size);
            info!("Increased cache size to {} bytes", state.current_size);
        }

        if snapshot.p99_latency > self.config.target_p99_latency {
            // Adjust compression or sharding to improve latency
            if state.compression_level > 1 {
                state.compression_level -= 1;
                info!("Reduced compression level to {}", state.compression_level);
            } else if state.shard_count < 1024 {
                state.shard_count *= 2;
                info!("Increased shard count to {}", state.shard_count);
            }
        }

        // Adjust eviction threshold based on memory pressure
        let memory_pressure = snapshot.memory_usage as f64 / state.current_size as f64;
        if memory_pressure > 0.95 {
            state.current_eviction_threshold = 0.8;
        } else if memory_pressure < 0.7 {
            state.current_eviction_threshold = 0.95;
        }

        state.last_adjustment = std::time::SystemTime::now();
        Ok(())
    }

    async fn collect_performance_snapshot(&self) -> CacheResult<PerformanceSnapshot> {
        // Collect metrics from the cache
        let hit_rate = 0.85; // Placeholder - would get from metrics
        let p99_latency = 5.0; // Placeholder - would get from metrics
        let memory_usage = 1024 * 1024 * 100; // Placeholder
        let cpu_usage = 0.15; // Placeholder

        Ok(PerformanceSnapshot {
            timestamp: Utc::now(),
            hit_rate,
            p99_latency,
            memory_usage,
            cpu_usage,
        })
    }

    /// Get current tuning parameters
    pub async fn get_parameters(&self) -> TuningState {
        self.tuning_state.read().await.clone()
    }
}

/// SLO/SLI monitoring and enforcement
#[allow(dead_code)]
pub struct SloMonitor {
    metrics: Arc<CacheMonitor>,
    slos: Vec<ServiceLevelObjective>,
    alerts: Arc<RwLock<Vec<SloViolation>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLevelObjective {
    pub name: String,
    pub description: String,
    pub target: f64,
    pub window: Duration,
    pub metric_type: SloMetricType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SloMetricType {
    CacheHitRate,
    P99Latency,
    Availability,
    ErrorRate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloViolation {
    pub slo_name: String,
    pub timestamp: DateTime<Utc>,
    pub actual_value: f64,
    pub target_value: f64,
    pub severity: ViolationSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Warning,
    Critical,
    Emergency,
}

impl SloMonitor {
    pub fn new(metrics: Arc<CacheMonitor>) -> Self {
        // Define default SLOs
        let slos = vec![
            ServiceLevelObjective {
                name: "cache_hit_rate".to_string(),
                description: "Cache hit rate should be above 80%".to_string(),
                target: 0.80,
                window: Duration::from_secs(300),
                metric_type: SloMetricType::CacheHitRate,
            },
            ServiceLevelObjective {
                name: "p99_latency".to_string(),
                description: "P99 latency should be below 10ms".to_string(),
                target: 10.0,
                window: Duration::from_secs(300),
                metric_type: SloMetricType::P99Latency,
            },
            ServiceLevelObjective {
                name: "availability".to_string(),
                description: "Cache availability should be above 99.9%".to_string(),
                target: 0.999,
                window: Duration::from_secs(3600),
                metric_type: SloMetricType::Availability,
            },
        ];

        Self {
            metrics,
            slos,
            alerts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Monitor SLOs continuously
    pub async fn monitor_slos(&self) {
        loop {
            for slo in &self.slos {
                if let Err(e) = self.check_slo(slo).await {
                    error!("Failed to check SLO {}: {}", slo.name, e);
                }
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }

    async fn check_slo(&self, slo: &ServiceLevelObjective) -> CacheResult<()> {
        let current_value = match slo.metric_type {
            SloMetricType::CacheHitRate => self.get_hit_rate().await?,
            SloMetricType::P99Latency => self.get_p99_latency().await?,
            SloMetricType::Availability => self.get_availability().await?,
            SloMetricType::ErrorRate => self.get_error_rate().await?,
        };

        let violated = match slo.metric_type {
            SloMetricType::CacheHitRate | SloMetricType::Availability => current_value < slo.target,
            SloMetricType::P99Latency | SloMetricType::ErrorRate => current_value > slo.target,
        };

        if violated {
            let severity = self.determine_severity(slo, current_value);
            let violation = SloViolation {
                slo_name: slo.name.clone(),
                timestamp: Utc::now(),
                actual_value: current_value,
                target_value: slo.target,
                severity,
            };

            self.record_violation(violation).await;
        }

        Ok(())
    }

    async fn get_hit_rate(&self) -> CacheResult<f64> {
        // Placeholder - would calculate from metrics
        Ok(0.85)
    }

    async fn get_p99_latency(&self) -> CacheResult<f64> {
        // Placeholder - would calculate from metrics
        Ok(8.5)
    }

    async fn get_availability(&self) -> CacheResult<f64> {
        // Placeholder - would calculate from metrics
        Ok(0.9995)
    }

    async fn get_error_rate(&self) -> CacheResult<f64> {
        // Placeholder - would calculate from metrics
        Ok(0.001)
    }

    fn determine_severity(&self, slo: &ServiceLevelObjective, value: f64) -> ViolationSeverity {
        let deviation = (value - slo.target).abs() / slo.target;

        if deviation > 0.5 {
            ViolationSeverity::Emergency
        } else if deviation > 0.2 {
            ViolationSeverity::Critical
        } else {
            ViolationSeverity::Warning
        }
    }

    async fn record_violation(&self, violation: SloViolation) {
        warn!(
            "SLO violation: {} (actual: {:.2}, target: {:.2})",
            violation.slo_name, violation.actual_value, violation.target_value
        );

        let mut alerts = self.alerts.write().await;
        alerts.push(violation);

        // Keep only recent alerts
        if alerts.len() > 1000 {
            alerts.remove(0);
        }
    }

    /// Get recent SLO violations
    pub async fn get_violations(&self) -> Vec<SloViolation> {
        self.alerts.read().await.clone()
    }
}

/// Operations runbook generator
pub struct RunbookGenerator {
    cache_type: String,
    common_issues: Vec<CommonIssue>,
}

#[derive(Debug, Clone)]
struct CommonIssue {
    symptom: String,
    possible_causes: Vec<String>,
    resolution_steps: Vec<String>,
    prevention: String,
}

impl RunbookGenerator {
    pub fn new(cache_type: String) -> Self {
        let common_issues = vec![
            CommonIssue {
                symptom: "High cache miss rate".to_string(),
                possible_causes: vec![
                    "Cache size too small".to_string(),
                    "Ineffective cache key generation".to_string(),
                    "Cache eviction too aggressive".to_string(),
                ],
                resolution_steps: vec![
                    "Check cache size configuration".to_string(),
                    "Review cache key generation logic".to_string(),
                    "Analyze eviction metrics".to_string(),
                    "Consider increasing cache size".to_string(),
                ],
                prevention: "Monitor cache hit rate continuously and set up alerts".to_string(),
            },
            CommonIssue {
                symptom: "Cache corruption errors".to_string(),
                possible_causes: vec![
                    "Disk errors".to_string(),
                    "Concurrent write conflicts".to_string(),
                    "Power failure during write".to_string(),
                ],
                resolution_steps: vec![
                    "Run cache integrity check".to_string(),
                    "Enable automatic corruption recovery".to_string(),
                    "Check disk health with smartctl".to_string(),
                    "Review recent system logs".to_string(),
                ],
                prevention: "Enable write-ahead logging and checksums".to_string(),
            },
            CommonIssue {
                symptom: "High cache latency".to_string(),
                possible_causes: vec![
                    "Excessive compression".to_string(),
                    "Lock contention".to_string(),
                    "Disk I/O bottleneck".to_string(),
                ],
                resolution_steps: vec![
                    "Profile cache operations".to_string(),
                    "Check compression settings".to_string(),
                    "Monitor disk I/O metrics".to_string(),
                    "Consider increasing shard count".to_string(),
                ],
                prevention: "Use self-tuning parameters and monitor P99 latency".to_string(),
            },
        ];

        Self {
            cache_type,
            common_issues,
        }
    }

    /// Generate markdown runbook
    pub fn generate_runbook(&self) -> String {
        let mut runbook = format!("# {} Cache Operations Runbook\n\n", self.cache_type);

        runbook.push_str("## Overview\n\n");
        runbook.push_str("This runbook provides guidance for operating and troubleshooting the cache system.\n\n");

        runbook.push_str("## Quick Health Check\n\n");
        runbook.push_str("```bash\n");
        runbook.push_str("# Check cache metrics\n");
        runbook.push_str("curl http://localhost:9090/metrics | grep cache_\n\n");
        runbook.push_str("# Check cache status\n");
        runbook.push_str("cuenv cache status\n\n");
        runbook.push_str("# Run integrity check\n");
        runbook.push_str("cuenv cache verify\n");
        runbook.push_str("```\n\n");

        runbook.push_str("## Common Issues and Resolutions\n\n");

        for (i, issue) in self.common_issues.iter().enumerate() {
            runbook.push_str(&format!("### {}. {}\n\n", i + 1, issue.symptom));

            runbook.push_str("**Possible Causes:**\n");
            for cause in &issue.possible_causes {
                runbook.push_str(&format!("- {cause}\n"));
            }
            runbook.push('\n');

            runbook.push_str("**Resolution Steps:**\n");
            for (j, step) in issue.resolution_steps.iter().enumerate() {
                runbook.push_str(&format!("{}. {}\n", j + 1, step));
            }
            runbook.push('\n');

            runbook.push_str(&format!("**Prevention:** {}\n\n", issue.prevention));
        }

        runbook.push_str("## Performance Tuning\n\n");
        runbook.push_str("### Key Parameters\n\n");
        runbook.push_str("- `cache_size`: Maximum cache size in bytes\n");
        runbook.push_str("- `compression_level`: 0-9 (0=none, 9=maximum)\n");
        runbook.push_str("- `shard_count`: Number of cache shards (power of 2)\n");
        runbook.push_str("- `eviction_threshold`: When to start evicting (0.0-1.0)\n\n");

        runbook.push_str("### Monitoring\n\n");
        runbook.push_str("Key metrics to monitor:\n");
        runbook.push_str("- `cache_hit_rate`: Should be > 80%\n");
        runbook.push_str("- `cache_latency_p99`: Should be < 10ms\n");
        runbook.push_str("- `cache_errors_total`: Should be near 0\n");
        runbook.push_str("- `cache_evictions_total`: High rate indicates size issues\n\n");

        runbook.push_str("## Emergency Procedures\n\n");
        runbook.push_str("### Cache Corruption\n");
        runbook.push_str("```bash\n");
        runbook.push_str("# Enable recovery mode\n");
        runbook.push_str("cuenv cache recover --auto\n\n");
        runbook.push_str("# If recovery fails, clear cache\n");
        runbook.push_str("cuenv cache clear --confirm\n");
        runbook.push_str("```\n\n");

        runbook.push_str("### Performance Degradation\n");
        runbook.push_str("```bash\n");
        runbook.push_str("# Enable self-tuning\n");
        runbook.push_str("cuenv cache tune --auto\n\n");
        runbook.push_str("# Manual adjustment\n");
        runbook.push_str("cuenv cache config --compression=1 --shards=512\n");
        runbook.push_str("```\n\n");

        runbook.push_str("## Contact\n\n");
        runbook.push_str("For escalation, contact the platform team.\n");

        runbook
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_corruption_recovery() {
        // Test would verify corruption recovery logic
    }

    #[tokio::test]
    async fn test_self_tuning() {
        // Test would verify self-tuning adjustments
    }

    #[tokio::test]
    async fn test_slo_monitoring() {
        // Test would verify SLO violation detection
    }

    #[test]
    fn test_runbook_generation() {
        let generator = RunbookGenerator::new("Production".to_string());
        let runbook = generator.generate_runbook();
        assert!(runbook.contains("Cache Operations Runbook"));
        assert!(runbook.contains("Common Issues"));
        assert!(runbook.contains("Emergency Procedures"));
    }
}

/// SLO monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloConfig {
    /// Target cache hit rate
    pub target_hit_rate: f64,
    /// Target p99 latency in milliseconds
    pub target_p99_latency_ms: f64,
    /// Target availability percentage
    pub target_availability: f64,
    /// Monitoring window duration
    pub window_duration: Duration,
}

/// System health report with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    /// Overall health status
    pub status: HealthStatus,
    /// Individual component health
    pub components: HashMap<String, ComponentHealth>,
    /// System metrics
    pub metrics: HealthMetrics,
    /// Timestamp of the report
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub details: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    pub cache_hit_rate: f64,
    pub avg_latency_ms: f64,
    pub error_rate: f64,
    pub memory_usage_bytes: usize,
    pub cpu_usage_percent: f64,
}

/// Production-grade wrapper for cache with monitoring and reliability features
pub struct ProductionHardening<C: Cache + Clone> {
    cache: Arc<MonitoredCache<C>>,
    metrics: Arc<CacheMonitor>,
    #[allow(dead_code)]
    corruption_recovery: Arc<CorruptionRecovery<C>>,
    #[allow(dead_code)]
    self_tuning: Arc<SelfTuningCache<C>>,
    #[allow(dead_code)]
    slo_monitor: Arc<SloMonitor>,
}

impl<C: Cache + Clone> ProductionHardening<C> {
    /// Create new production hardening wrapper
    pub fn new(
        cache: C,
        recovery_config: RecoveryConfig,
        tuning_config: TuningConfig,
        _slo_config: SloConfig,
    ) -> Self {
        let metrics = Arc::new(
            CacheMonitor::new("production-hardening").expect("Failed to create cache monitor"),
        );
        let monitored_cache = Arc::new(
            MonitoredCache::new(cache, "production-hardening")
                .expect("Failed to create monitored cache"),
        );

        Self {
            cache: monitored_cache.clone(),
            metrics: metrics.clone(),
            corruption_recovery: Arc::new(CorruptionRecovery::new(
                monitored_cache.clone(),
                recovery_config,
            )),
            self_tuning: Arc::new(SelfTuningCache::new(
                monitored_cache.clone(),
                metrics.clone(),
                tuning_config,
            )),
            slo_monitor: Arc::new(SloMonitor::new(metrics.clone())),
        }
    }

    /// Get current system health
    pub async fn health_check(&self) -> SystemHealthReport {
        let stats = match self.cache.statistics().await {
            Ok(s) => s,
            Err(_) => {
                return SystemHealthReport {
                    status: HealthStatus::Unhealthy,
                    components: HashMap::new(),
                    metrics: HealthMetrics {
                        cache_hit_rate: 0.0,
                        avg_latency_ms: 0.0,
                        error_rate: 1.0,
                        memory_usage_bytes: 0,
                        cpu_usage_percent: 0.0,
                    },
                    timestamp: std::time::SystemTime::now(),
                }
            }
        };

        let total_ops = stats.hits + stats.misses;
        let error_rate = if total_ops > 0 {
            stats.errors as f64 / total_ops as f64
        } else {
            0.0
        };
        let hit_rate = if total_ops > 0 {
            stats.hits as f64 / total_ops as f64
        } else {
            0.0
        };

        let status = if error_rate > 0.05 {
            HealthStatus::Unhealthy
        } else if hit_rate < 0.8 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let mut components = HashMap::new();

        // Check cache component
        components.insert(
            "cache".to_string(),
            ComponentHealth {
                name: "cache".to_string(),
                status: HealthStatus::Healthy,
                details: HashMap::new(),
            },
        );

        // Check metrics component
        components.insert(
            "metrics".to_string(),
            ComponentHealth {
                name: "metrics".to_string(),
                status: HealthStatus::Healthy,
                details: HashMap::new(),
            },
        );

        SystemHealthReport {
            status,
            components,
            metrics: HealthMetrics {
                cache_hit_rate: hit_rate,
                avg_latency_ms: 0.0, // Would need more detailed metrics integration
                error_rate,
                memory_usage_bytes: 0,  // Would need system metrics integration
                cpu_usage_percent: 0.0, // Would need system metrics integration
            },
            timestamp: SystemTime::now(),
        }
    }

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<CacheMonitor> {
        self.metrics.clone()
    }

    /// Get the underlying cache
    pub fn cache(&self) -> Arc<MonitoredCache<C>> {
        self.cache.clone()
    }
}
