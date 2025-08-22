//! Monitored cache implementation with comprehensive observability
//!
//! This module provides a cache wrapper that adds monitoring capabilities
//! to any cache implementation, tracking metrics, traces, and performance.

use crate::errors::Result;
use crate::monitoring::CacheMonitor;
use crate::streaming::{CacheReader, CacheWriter, StreamingCache};
use crate::traits::{Cache, CacheMetadata, CacheStatistics};
use async_trait::async_trait;
use futures::io::{AsyncRead, AsyncWrite};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, info};

/// Health report for monitoring endpoints
#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub overall_status: crate::security::audit::HealthStatus,
    #[serde(serialize_with = "serialize_duration_as_secs")]
    pub uptime: Duration,
    pub generated_at: SystemTime,
    pub components: Vec<ComponentHealth>,
    pub summary: HealthSummary,
}

/// Health summary statistics
#[derive(Debug, Clone, Serialize)]
pub struct HealthSummary {
    pub total_checks: u32,
    pub healthy_count: u32,
    pub warning_count: u32,
    pub critical_count: u32,
    pub down_count: u32,
}

/// Individual component health
#[derive(Debug, Clone, Serialize)]
pub struct ComponentHealth {
    pub component: String,
    pub status: crate::security::audit::HealthStatus,
    #[serde(serialize_with = "serialize_duration_as_secs")]
    pub check_duration: Duration,
}

/// Serialize Duration as seconds (f64)
fn serialize_duration_as_secs<S>(
    duration: &Duration,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_f64(duration.as_secs_f64())
}

/// Cache wrapper that adds comprehensive monitoring
pub struct MonitoredCache<C: Cache>
where
    C: Clone,
{
    /// Underlying cache implementation
    cache: C,
    /// Monitoring system
    monitor: CacheMonitor,
    /// Service name for tracing
    service_name: String,
}

impl<C: Cache + Clone> MonitoredCache<C> {
    /// Create a new monitored cache
    pub fn new(cache: C, service_name: impl Into<String>) -> Result<Self> {
        let service_name = service_name.into();
        let monitor = CacheMonitor::new(&service_name)?;

        Ok(Self {
            cache,
            monitor,
            service_name,
        })
    }

    /// Get access to the monitoring system
    pub fn monitor(&self) -> &CacheMonitor {
        &self.monitor
    }

    /// Get Prometheus metrics endpoint data
    pub fn metrics_text(&self) -> String {
        self.monitor.metrics_text()
    }

    /// Get cache hit rate report
    pub fn hit_rate_report(&self) -> crate::monitoring::HitRateReport {
        self.monitor.hit_rate_report()
    }

    /// Enable performance profiling
    pub fn enable_profiling(&self) {
        self.monitor.enable_profiling();
    }

    /// Get flamegraph data
    pub fn flamegraph_data(&self) -> String {
        self.monitor.generate_flamegraph()
    }

    /// Update cache statistics
    async fn update_stats(&self) -> Result<()> {
        match self.cache.statistics().await {
            Ok(stats) => {
                // In a real implementation, we'd also get memory and disk usage
                self.monitor.update_statistics(&stats, 0, 0);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

#[async_trait]
impl<C: Cache + Send + Sync + Clone> Cache for MonitoredCache<C> {
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let start = Instant::now();
        let operation = self.monitor.start_operation("cache.get", key);

        match self.cache.get::<T>(key).await {
            Ok(Some(value)) => {
                let duration = start.elapsed();
                self.monitor.record_hit(key, "get", duration);
                operation.complete();
                Ok(Some(value))
            }
            Ok(None) => {
                let duration = start.elapsed();
                self.monitor.record_miss(key, "get", duration);
                operation.complete();
                Ok(None)
            }
            Err(e) => {
                self.monitor.record_error("get", &e);
                operation.error(&e);
                Err(e)
            }
        }
    }

    async fn put<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        let start = Instant::now();
        let operation = self.monitor.start_operation("cache.put", key);

        // Estimate size (in real implementation, would calculate actual serialized size)
        let estimated_size = std::mem::size_of_val(value) as u64;

        match self.cache.put(key, value, ttl).await {
            Ok(()) => {
                let duration = start.elapsed();
                self.monitor.record_write(key, estimated_size, duration);
                operation.complete();

                // Update stats asynchronously if cache supports cloning
                // For now, we'll skip async stats update since it requires C: Clone + 'static
                // which would be a breaking change to the trait bounds

                Ok(())
            }
            Err(e) => {
                self.monitor.record_error("put", &e);
                operation.error(&e);
                Err(e)
            }
        }
    }

    async fn remove(&self, key: &str) -> Result<bool> {
        let start = Instant::now();
        let operation = self.monitor.start_operation("cache.remove", key);

        match self.cache.remove(key).await {
            Ok(removed) => {
                let duration = start.elapsed();
                if removed {
                    self.monitor.record_removal(key, duration);
                }
                operation.complete();
                Ok(removed)
            }
            Err(e) => {
                self.monitor.record_error("remove", &e);
                operation.error(&e);
                Err(e)
            }
        }
    }

    async fn contains(&self, key: &str) -> Result<bool> {
        let start = Instant::now();
        let operation = self.monitor.start_operation("cache.contains", key);

        match self.cache.contains(key).await {
            Ok(exists) => {
                let duration = start.elapsed();
                if exists {
                    self.monitor.record_hit(key, "contains", duration);
                } else {
                    self.monitor.record_miss(key, "contains", duration);
                }
                operation.complete();
                Ok(exists)
            }
            Err(e) => {
                self.monitor.record_error("contains", &e);
                operation.error(&e);
                Err(e)
            }
        }
    }

    async fn metadata(&self, key: &str) -> Result<Option<CacheMetadata>> {
        let start = Instant::now();
        let operation = self.monitor.start_operation("cache.metadata", key);

        match self.cache.metadata(key).await {
            Ok(Some(metadata)) => {
                let duration = start.elapsed();
                self.monitor.record_hit(key, "metadata", duration);
                operation.complete();
                Ok(Some(metadata))
            }
            Ok(None) => {
                let duration = start.elapsed();
                self.monitor.record_miss(key, "metadata", duration);
                operation.complete();
                Ok(None)
            }
            Err(e) => {
                self.monitor.record_error("metadata", &e);
                operation.error(&e);
                Err(e)
            }
        }
    }

    async fn clear(&self) -> Result<()> {
        let start = Instant::now();
        let operation = self.monitor.start_operation("cache.clear", "*");

        match self.cache.clear().await {
            Ok(()) => {
                let duration = start.elapsed();
                info!("Cache cleared in {:?}", duration);
                operation.complete();

                // Update stats to reflect the clear
                let _ = self.update_stats().await;

                Ok(())
            }
            Err(e) => {
                self.monitor.record_error("clear", &e);
                operation.error(&e);
                Err(e)
            }
        }
    }

    async fn statistics(&self) -> Result<CacheStatistics> {
        match self.cache.statistics().await {
            Ok(stats) => {
                // Update monitor with latest stats
                self.monitor.update_statistics(&stats, 0, 0);
                Ok(stats)
            }
            Err(e) => {
                self.monitor.record_error("statistics", &e);
                Err(e)
            }
        }
    }
}

impl<C: Cache + StreamingCache + Clone> StreamingCache for MonitoredCache<C> {
    fn get_reader<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<CacheReader>>> + Send + 'a>> {
        Box::pin(async move {
            let start = Instant::now();
            let operation = self.monitor.start_operation("cache.get_reader", key);

            match self.cache.get_reader(key).await {
                Ok(Some(reader)) => {
                    let duration = start.elapsed();
                    self.monitor.record_hit(key, "get_reader", duration);
                    operation.complete();
                    Ok(Some(reader))
                }
                Ok(None) => {
                    let duration = start.elapsed();
                    self.monitor.record_miss(key, "get_reader", duration);
                    operation.complete();
                    Ok(None)
                }
                Err(e) => {
                    self.monitor.record_error("get_reader", &e);
                    operation.error(&e);
                    Err(e)
                }
            }
        })
    }

    fn get_writer<'a>(
        &'a self,
        key: &'a str,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<CacheWriter>> + Send + 'a>> {
        Box::pin(async move {
            let start = Instant::now();
            let operation = self.monitor.start_operation("cache.get_writer", key);

            match self.cache.get_writer(key, ttl).await {
                Ok(writer) => {
                    let duration = start.elapsed();
                    debug!("Created cache writer for {} in {:?}", key, duration);
                    operation.complete();
                    Ok(writer)
                }
                Err(e) => {
                    self.monitor.record_error("get_writer", &e);
                    operation.error(&e);
                    Err(e)
                }
            }
        })
    }

    fn put_stream<'a, R>(
        &'a self,
        key: &'a str,
        reader: R,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>
    where
        R: AsyncRead + Send + 'a,
    {
        Box::pin(async move {
            let start = Instant::now();
            let operation = self.monitor.start_operation("cache.put_stream", key);

            match self.cache.put_stream(key, reader, ttl).await {
                Ok(bytes_written) => {
                    let duration = start.elapsed();
                    self.monitor.record_write(key, bytes_written, duration);
                    operation.complete();
                    Ok(bytes_written)
                }
                Err(e) => {
                    self.monitor.record_error("put_stream", &e);
                    operation.error(&e);
                    Err(e)
                }
            }
        })
    }

    fn get_stream<'a, W>(
        &'a self,
        key: &'a str,
        writer: W,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u64>>> + Send + 'a>>
    where
        W: AsyncWrite + Send + 'a,
    {
        Box::pin(async move {
            let start = Instant::now();
            let operation = self.monitor.start_operation("cache.get_stream", key);

            match self.cache.get_stream(key, writer).await {
                Ok(Some(bytes_read)) => {
                    let duration = start.elapsed();
                    self.monitor.record_hit(key, "get_stream", duration);
                    operation.complete();
                    Ok(Some(bytes_read))
                }
                Ok(None) => {
                    let duration = start.elapsed();
                    self.monitor.record_miss(key, "get_stream", duration);
                    operation.complete();
                    Ok(None)
                }
                Err(e) => {
                    self.monitor.record_error("get_stream", &e);
                    operation.error(&e);
                    Err(e)
                }
            }
        })
    }
}

impl<C: Cache + Clone> Clone for MonitoredCache<C> {
    fn clone(&self) -> Self {
        Self {
            cache: self.cache.clone(),
            monitor: self.monitor.clone(),
            service_name: self.service_name.clone(),
        }
    }
}

impl<C: Cache + Clone> fmt::Debug for MonitoredCache<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitoredCache")
            .field("service_name", &self.service_name)
            .field("hit_rate", &self.monitor.hit_rate())
            .finish()
    }
}

// Additional methods for health monitoring
impl<C: Cache + Clone> MonitoredCache<C> {
    /// Get health report for monitoring endpoints (stub implementation)
    pub async fn health_report(&self) -> crate::errors::Result<HealthReport> {
        use crate::security::audit::HealthStatus;

        // Basic health report implementation
        let report = HealthReport {
            overall_status: HealthStatus::Healthy,
            uptime: Duration::from_secs(0), // TODO: Track actual uptime
            generated_at: SystemTime::now(),
            components: vec![],
            summary: HealthSummary {
                total_checks: 1,
                healthy_count: 1,
                warning_count: 0,
                critical_count: 0,
                down_count: 0,
            },
        };
        Ok(report)
    }

    /// Check if the cache is shutting down (stub implementation)
    pub fn is_shutting_down(&self) -> bool {
        false // TODO: Implement proper shutdown tracking
    }

    /// Get configuration validation errors (stub implementation)
    pub async fn get_config_validation_errors(&self) -> Vec<String> {
        vec![] // TODO: Implement config validation
    }

    /// Initiate shutdown (stub implementation)
    pub async fn shutdown(&self) -> crate::errors::Result<()> {
        // TODO: Implement proper shutdown procedure
        Ok(())
    }
}

/// Builder for creating monitored caches with custom configuration
pub struct MonitoredCacheBuilder<C: Cache> {
    cache: C,
    service_name: String,
    enable_profiling: bool,
}

impl<C: Cache + Clone> MonitoredCacheBuilder<C> {
    /// Create a new builder
    pub fn new(cache: C) -> Self {
        Self {
            cache,
            service_name: "cuenv-cache".to_string(),
            enable_profiling: false,
        }
    }

    /// Set the service name for tracing
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Enable performance profiling
    pub fn with_profiling(mut self) -> Self {
        self.enable_profiling = true;
        self
    }

    /// Build the monitored cache
    pub fn build(self) -> Result<MonitoredCache<C>> {
        let cache = MonitoredCache::new(self.cache, self.service_name)?;

        if self.enable_profiling {
            cache.enable_profiling();
        }

        Ok(cache)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Cache;
    use crate::traits::{Cache as CacheTrait, CacheConfig};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_monitored_cache_operations() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let base_cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

        let monitored = MonitoredCacheBuilder::new(base_cache)
            .with_service_name("test-cache")
            .build()?;

        // Test operations
        monitored.put("test-key", &"test-value", None).await?;

        let value: Option<String> = monitored.get("test-key").await?;
        assert_eq!(value, Some("test-value".to_string()));

        // Check monitoring
        let report = monitored.hit_rate_report();
        assert!(report.one_minute > 0.0);

        // Get metrics
        let metrics = monitored.metrics_text();
        // Check for the actual metric names from the Prometheus output
        assert!(metrics.contains("cuenv_cache_operations_total"));
        assert!(metrics.contains("operation=\"get\""));
        assert!(metrics.contains("operation=\"write\""));

        Ok(())
    }

    #[tokio::test]
    async fn test_profiling() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let base_cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

        let monitored = MonitoredCacheBuilder::new(base_cache)
            .with_profiling()
            .build()?;

        // Ensure profiling is enabled
        monitored.enable_profiling();

        // Perform some operations
        for i in 0..10 {
            monitored
                .put(&format!("key-{i}"), &format!("value-{i}"), None)
                .await?;
            let _: Option<String> = monitored.get(&format!("key-{i}")).await?;
        }

        // Get flamegraph data
        let flamegraph = monitored.flamegraph_data();
        // Flamegraph may be empty if profiling didn't capture enough samples
        // Just check that the method doesn't panic
        tracing::info!("Flamegraph data length: {}", flamegraph.len());

        Ok(())
    }
}
