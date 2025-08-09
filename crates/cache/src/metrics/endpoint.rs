//! Metrics endpoint provider for cache observability
//!
//! This module provides a simple way to access cache metrics and reports
//! without requiring a full HTTP server.

use crate::monitored::MonitoredCache;
use crate::traits::Cache;
use std::sync::Arc;

/// Metrics endpoint provider for cache observability data
pub struct MetricsEndpoint<C: Cache + Clone> {
    cache: Arc<MonitoredCache<C>>,
}

impl<C: Cache + Clone> MetricsEndpoint<C> {
    /// Create a new metrics endpoint
    pub fn new(cache: MonitoredCache<C>) -> Self {
        Self {
            cache: Arc::new(cache),
        }
    }

    /// Get Prometheus metrics as text
    pub fn prometheus_metrics(&self) -> String {
        self.cache.metrics_text()
    }

    /// Get hit rate analysis as JSON
    pub fn hit_rate_json(&self) -> Result<String, serde_json::Error> {
        let report = self.cache.hit_rate_report();
        serde_json::to_string_pretty(&HitRateJson::from(report))
    }

    /// Get real-time statistics as JSON
    pub fn stats_json(&self) -> Result<String, serde_json::Error> {
        let stats = self.cache.monitor().real_time_stats();
        serde_json::to_string_pretty(&StatsJson::from(stats))
    }

    /// Get flamegraph data
    pub fn flamegraph_data(&self) -> String {
        self.cache.flamegraph_data()
    }

    /// Get health status
    pub fn health_status(&self) -> &'static str {
        "OK"
    }
}

impl<C: Cache + Clone> Clone for MetricsEndpoint<C> {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
        }
    }
}

// JSON representations for the API

#[derive(serde::Serialize)]
struct HitRateJson {
    time_windows: TimeWindowsJson,
    key_patterns: Vec<PatternStatsJson>,
    operation_types: Vec<OperationStatsJson>,
}

#[derive(serde::Serialize)]
struct TimeWindowsJson {
    one_minute: f64,
    five_minutes: f64,
    one_hour: f64,
    one_day: f64,
}

#[derive(serde::Serialize)]
struct PatternStatsJson {
    pattern: String,
    hit_rate: f64,
    total_accesses: u64,
}

#[derive(serde::Serialize)]
struct OperationStatsJson {
    operation: String,
    hit_rate: f64,
    total_calls: u64,
}

#[derive(serde::Serialize)]
struct StatsJson {
    operations_in_flight: u64,
    avg_response_time_us: u64,
    p99_response_time_us: u64,
}

impl From<crate::monitoring::HitRateReport> for HitRateJson {
    fn from(report: crate::monitoring::HitRateReport) -> Self {
        Self {
            time_windows: TimeWindowsJson {
                one_minute: report.one_minute,
                five_minutes: report.five_minutes,
                one_hour: report.one_hour,
                one_day: report.one_day,
            },
            key_patterns: report
                .key_patterns
                .into_iter()
                .map(|p| PatternStatsJson {
                    pattern: p.pattern,
                    hit_rate: p.hit_rate,
                    total_accesses: p.total_accesses,
                })
                .collect(),
            operation_types: report
                .operation_types
                .into_iter()
                .map(|o| OperationStatsJson {
                    operation: o.operation,
                    hit_rate: o.hit_rate,
                    total_calls: o.total_calls,
                })
                .collect(),
        }
    }
}

impl From<crate::monitoring::RealTimeStatsReport> for StatsJson {
    fn from(stats: crate::monitoring::RealTimeStatsReport) -> Self {
        Self {
            operations_in_flight: stats.operations_in_flight,
            avg_response_time_us: stats.avg_response_time_us,
            p99_response_time_us: stats.p99_response_time_us,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Cache;
    use crate::monitored::MonitoredCacheBuilder;
    use crate::traits::{Cache as CacheTrait, CacheConfig};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_metrics_endpoint() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new().unwrap();
        let base_cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

        let monitored = MonitoredCacheBuilder::new(base_cache)
            .with_service_name("test-endpoint")
            .build()?;

        // Perform some operations
        monitored.put("test-key", &"test-value", None).await?;
        let _: Option<String> = monitored.get("test-key").await?;

        let endpoint = MetricsEndpoint::new(monitored);

        // Test Prometheus metrics
        let prometheus = endpoint.prometheus_metrics();
        assert!(prometheus.contains("cuenv_cache_operations_total"));

        // Test hit rate JSON
        let hit_rate = endpoint.hit_rate_json()?;
        assert!(hit_rate.contains("time_windows"));

        // Test stats JSON
        let stats = endpoint.stats_json()?;
        assert!(stats.contains("operations_in_flight"));

        // Test health
        assert_eq!(endpoint.health_status(), "OK");

        Ok(())
    }
}
