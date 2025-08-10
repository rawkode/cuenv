//! Metrics event subscriber for performance monitoring

use crate::events::{EnhancedEvent, EventSubscriber, SystemEvent, TaskEvent, PipelineEvent, CacheEvent};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error};

/// Metrics subscriber for performance monitoring and observability
pub struct MetricsSubscriber {
    /// Metrics storage
    metrics: Arc<RwLock<MetricsStore>>,
    /// Atomic counters for high-frequency metrics
    counters: MetricsCounters,
    /// Whether to expose metrics via HTTP endpoint
    expose_http: bool,
    /// HTTP server port (if enabled)
    http_port: Option<u16>,
}

/// Thread-safe atomic counters for high-frequency metrics
#[derive(Debug)]
struct MetricsCounters {
    /// Total events processed
    events_total: AtomicU64,
    /// Task events processed
    task_events: AtomicU64,
    /// Pipeline events processed
    pipeline_events: AtomicU64,
    /// Cache events processed
    cache_events: AtomicU64,
    /// Total tasks completed successfully
    tasks_completed_success: AtomicU64,
    /// Total tasks failed
    tasks_completed_failure: AtomicU64,
    /// Total tasks skipped
    tasks_skipped: AtomicU64,
    /// Cache hits
    cache_hits: AtomicU64,
    /// Cache misses
    cache_misses: AtomicU64,
}

/// Configuration for metrics storage limits
const MAX_TASK_DURATIONS_PER_TASK: usize = 1000; // Keep last 1000 measurements per task
const MAX_PIPELINE_DURATIONS: usize = 500; // Keep last 500 pipeline executions
const CLEANUP_THRESHOLD_WRITES: u64 = 10000; // Clean up every 10k metric writes

/// Detailed metrics storage with bounded collections
#[derive(Debug)]
struct MetricsStore {
    /// Task execution times (task_name -> durations in ms) - bounded sliding window
    task_durations: HashMap<String, std::collections::VecDeque<u64>>,
    /// Task success rates (task_name -> (success_count, total_count))
    task_success_rates: HashMap<String, (u64, u64)>,
    /// Pipeline execution times - bounded sliding window
    pipeline_durations: std::collections::VecDeque<u64>,
    /// Cache operation types and their counts
    cache_operations: HashMap<String, u64>,
    /// System performance metrics
    system_metrics: SystemMetrics,
    /// Last metric update time
    last_update: Option<SystemTime>,
    /// Write counter for periodic cleanup
    write_counter: u64,
}

/// System-level performance metrics
#[derive(Debug, Default)]
struct SystemMetrics {
    /// Average task execution time (ms)
    avg_task_duration: f64,
    /// Task throughput (tasks per second)
    task_throughput: f64,
    /// Cache hit ratio (0.0 to 1.0)
    cache_hit_ratio: f64,
    /// Event processing rate (events per second)
    event_rate: f64,
}

/// Metrics summary for external consumption
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSummary {
    pub events_total: u64,
    pub tasks_completed_success: u64,
    pub tasks_completed_failure: u64,
    pub tasks_skipped: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_ratio: f64,
    pub avg_task_duration_ms: f64,
    pub task_throughput_per_second: f64,
    pub top_slowest_tasks: Vec<(String, f64)>, // (task_name, avg_duration_ms)
    pub most_failed_tasks: Vec<(String, u64)>, // (task_name, failure_count)
    pub last_update: Option<u64>, // Unix timestamp
}

impl MetricsSubscriber {
    /// Create a new metrics subscriber
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(MetricsStore::default())),
            counters: MetricsCounters::new(),
            expose_http: false,
            http_port: None,
        }
    }

    /// Create a metrics subscriber with HTTP endpoint enabled
    pub fn with_http_endpoint(port: u16) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(MetricsStore::default())),
            counters: MetricsCounters::new(),
            expose_http: true,
            http_port: Some(port),
        }
    }

    /// Get current metrics summary
    pub async fn get_metrics_summary(&self) -> MetricsSummary {
        let metrics = self.metrics.read().await;
        
        // Calculate cache hit ratio
        let total_cache_ops = self.counters.cache_hits.load(Ordering::Relaxed) 
            + self.counters.cache_misses.load(Ordering::Relaxed);
        let cache_hit_ratio = if total_cache_ops > 0 {
            self.counters.cache_hits.load(Ordering::Relaxed) as f64 / total_cache_ops as f64
        } else {
            0.0
        };

        // Find top slowest tasks
        let mut task_avg_durations: Vec<(String, f64)> = metrics.task_durations
            .iter()
            .map(|(name, durations)| {
                let avg = durations.iter().sum::<u64>() as f64 / durations.len() as f64;
                (name.clone(), avg)
            })
            .collect();
        task_avg_durations.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        task_avg_durations.truncate(10); // Top 10

        // Find most failed tasks
        let mut failed_tasks: Vec<(String, u64)> = metrics.task_success_rates
            .iter()
            .filter_map(|(name, (success, total))| {
                let failures = total - success;
                if failures > 0 {
                    Some((name.clone(), failures))
                } else {
                    None
                }
            })
            .collect();
        failed_tasks.sort_by(|a, b| b.1.cmp(&a.1));
        failed_tasks.truncate(10); // Top 10

        MetricsSummary {
            events_total: self.counters.events_total.load(Ordering::Relaxed),
            tasks_completed_success: self.counters.tasks_completed_success.load(Ordering::Relaxed),
            tasks_completed_failure: self.counters.tasks_completed_failure.load(Ordering::Relaxed),
            tasks_skipped: self.counters.tasks_skipped.load(Ordering::Relaxed),
            cache_hits: self.counters.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.counters.cache_misses.load(Ordering::Relaxed),
            cache_hit_ratio,
            avg_task_duration_ms: metrics.system_metrics.avg_task_duration,
            task_throughput_per_second: metrics.system_metrics.task_throughput,
            top_slowest_tasks: task_avg_durations,
            most_failed_tasks: failed_tasks,
            last_update: metrics.last_update.and_then(|t| t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())),
        }
    }

    /// Process a task event for metrics
    async fn process_task_event(&self, event: &TaskEvent) {
        self.counters.task_events.fetch_add(1, Ordering::Relaxed);
        
        let mut metrics = self.metrics.write().await;
        metrics.last_update = Some(SystemTime::now());

        match event {
            TaskEvent::TaskCompleted { task_name, duration_ms, .. } => {
                self.counters.tasks_completed_success.fetch_add(1, Ordering::Relaxed);
                
                // Record task duration with bounded storage
                let durations = metrics.task_durations
                    .entry(task_name.clone())
                    .or_insert_with(|| std::collections::VecDeque::with_capacity(MAX_TASK_DURATIONS_PER_TASK));
                
                durations.push_back(*duration_ms);
                
                // Remove oldest if we exceed limit
                if durations.len() > MAX_TASK_DURATIONS_PER_TASK {
                    durations.pop_front();
                }

                // Update success rate
                let (success, total) = metrics.task_success_rates
                    .entry(task_name.clone())
                    .or_insert((0, 0));
                *success += 1;
                *total += 1;
            }
            TaskEvent::TaskFailed { task_name, .. } => {
                self.counters.tasks_completed_failure.fetch_add(1, Ordering::Relaxed);
                
                // Update success rate (failure)
                let (_, total) = metrics.task_success_rates
                    .entry(task_name.clone())
                    .or_insert((0, 0));
                *total += 1;
            }
            TaskEvent::TaskSkipped { .. } => {
                self.counters.tasks_skipped.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        // Recalculate system metrics periodically
        self.update_system_metrics(&mut metrics).await;
    }

    /// Process a pipeline event for metrics
    async fn process_pipeline_event(&self, event: &PipelineEvent) {
        self.counters.pipeline_events.fetch_add(1, Ordering::Relaxed);
        
        if let PipelineEvent::PipelineCompleted { total_duration_ms, .. } = event {
            let mut metrics = self.metrics.write().await;
            
            // Add to bounded pipeline durations
            metrics.pipeline_durations.push_back(*total_duration_ms);
            
            // Remove oldest if we exceed limit
            if metrics.pipeline_durations.len() > MAX_PIPELINE_DURATIONS {
                metrics.pipeline_durations.pop_front();
            }
            
            metrics.last_update = Some(SystemTime::now());
            
            // Perform periodic cleanup
            metrics.write_counter += 1;
            if metrics.write_counter % CLEANUP_THRESHOLD_WRITES == 0 {
                self.cleanup_old_metrics(&mut metrics).await;
            }
        }
    }

    /// Process a cache event for metrics
    async fn process_cache_event(&self, event: &CacheEvent) {
        self.counters.cache_events.fetch_add(1, Ordering::Relaxed);
        
        match event {
            CacheEvent::CacheHit { .. } => {
                self.counters.cache_hits.fetch_add(1, Ordering::Relaxed);
            }
            CacheEvent::CacheMiss { .. } => {
                self.counters.cache_misses.fetch_add(1, Ordering::Relaxed);
            }
            CacheEvent::CacheWrite { .. } => {
                let mut metrics = self.metrics.write().await;
                *metrics.cache_operations.entry("write".to_string()).or_insert(0) += 1;
                metrics.last_update = Some(SystemTime::now());
            }
            CacheEvent::CacheEvict { .. } => {
                let mut metrics = self.metrics.write().await;
                *metrics.cache_operations.entry("evict".to_string()).or_insert(0) += 1;
                metrics.last_update = Some(SystemTime::now());
            }
        }
    }

    /// Update system-level performance metrics
    async fn update_system_metrics(&self, metrics: &mut MetricsStore) {
        // Calculate average task duration
        let all_durations: Vec<u64> = metrics.task_durations
            .values()
            .flatten()
            .cloned()
            .collect();
        
        if !all_durations.is_empty() {
            metrics.system_metrics.avg_task_duration = 
                all_durations.iter().sum::<u64>() as f64 / all_durations.len() as f64;
        }

        // Calculate cache hit ratio
        let cache_hits = self.counters.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.counters.cache_misses.load(Ordering::Relaxed);
        let total_cache_ops = cache_hits + cache_misses;
        
        if total_cache_ops > 0 {
            metrics.system_metrics.cache_hit_ratio = cache_hits as f64 / total_cache_ops as f64;
        }

        // Calculate event processing rate (simplified)
        let events_total = self.counters.events_total.load(Ordering::Relaxed);
        if events_total > 0 {
            metrics.system_metrics.event_rate = events_total as f64 / 60.0; // Very rough estimate
        }

        debug!(
            avg_duration_ms = metrics.system_metrics.avg_task_duration,
            cache_hit_ratio = metrics.system_metrics.cache_hit_ratio,
            events_total = events_total,
            task_count = metrics.task_durations.len(),
            pipeline_count = metrics.pipeline_durations.len(),
            "Updated system metrics"
        );
    }
    
    /// Clean up old metrics to prevent unbounded growth
    async fn cleanup_old_metrics(&self, metrics: &mut MetricsStore) {
        let now = SystemTime::now();
        let cleanup_threshold = Duration::from_secs(24 * 60 * 60 * 7); // 7 days
        
        // Clean up tasks that haven't been updated recently
        // This is a simplified cleanup - in practice you might want more sophisticated retention policies
        if let Some(last_update) = metrics.last_update {
            if now.duration_since(last_update).unwrap_or(Duration::ZERO) > cleanup_threshold {
                // Reset counters for very old data
                metrics.cache_operations.retain(|_, count| *count > 0);
                
                debug!(
                    tasks_retained = metrics.task_durations.len(),
                    cache_ops_retained = metrics.cache_operations.len(),
                    "Cleaned up old metrics"
                );
            }
        }
        
        // Ensure all task duration collections stay within bounds
        for (task_name, durations) in &mut metrics.task_durations {
            if durations.len() > MAX_TASK_DURATIONS_PER_TASK {
                let excess = durations.len() - MAX_TASK_DURATIONS_PER_TASK;
                for _ in 0..excess {
                    durations.pop_front();
                }
                debug!(
                    task = task_name,
                    retained_measurements = durations.len(),
                    "Cleaned up excess task duration measurements"
                );
            }
        }
    }

    /// Reset all metrics (useful for testing)
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = MetricsStore::default();
        
        // Reset atomic counters  
        self.counters.events_total.store(0, Ordering::Relaxed);
        self.counters.task_events.store(0, Ordering::Relaxed);
        self.counters.pipeline_events.store(0, Ordering::Relaxed);
        self.counters.cache_events.store(0, Ordering::Relaxed);
        self.counters.tasks_completed_success.store(0, Ordering::Relaxed);
        self.counters.tasks_completed_failure.store(0, Ordering::Relaxed);
        self.counters.tasks_skipped.store(0, Ordering::Relaxed);
        self.counters.cache_hits.store(0, Ordering::Relaxed);
        self.counters.cache_misses.store(0, Ordering::Relaxed);
        
        debug!("All metrics reset");
    }
}

impl MetricsCounters {
    fn new() -> Self {
        Self {
            events_total: AtomicU64::new(0),
            task_events: AtomicU64::new(0),
            pipeline_events: AtomicU64::new(0),
            cache_events: AtomicU64::new(0),
            tasks_completed_success: AtomicU64::new(0),
            tasks_completed_failure: AtomicU64::new(0),
            tasks_skipped: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }
}

impl Default for MetricsStore {
    fn default() -> Self {
        Self {
            task_durations: HashMap::new(),
            task_success_rates: HashMap::new(),
            pipeline_durations: std::collections::VecDeque::with_capacity(MAX_PIPELINE_DURATIONS),
            cache_operations: HashMap::new(),
            system_metrics: SystemMetrics::default(),
            last_update: None,
            write_counter: 0,
        }
    }
}

impl Default for MetricsSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventSubscriber for MetricsSubscriber {
    async fn handle_event(&self, event: &EnhancedEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.counters.events_total.fetch_add(1, Ordering::Relaxed);

        match &event.event {
            SystemEvent::Task(task_event) => {
                self.process_task_event(task_event).await;
            }
            SystemEvent::Pipeline(pipeline_event) => {
                self.process_pipeline_event(pipeline_event).await;
            }
            SystemEvent::Cache(cache_event) => {
                self.process_cache_event(cache_event).await;
            }
            SystemEvent::Env(_) | SystemEvent::Dependency(_) => {
                // These events don't need special metric processing for now
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "metrics"
    }

    fn is_interested(&self, _event: &SystemEvent) -> bool {
        // Metrics subscriber is interested in all events
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{SystemEvent, TaskEvent, PipelineEvent, CacheEvent};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_metrics_subscriber_creation() {
        let subscriber = MetricsSubscriber::new();
        assert_eq!(subscriber.name(), "metrics");
        assert!(subscriber.is_interested(&SystemEvent::Task(TaskEvent::TaskStarted {
            task_name: "test".to_string(),
            task_id: "test-1".to_string(),
        })));
    }

    #[tokio::test]
    async fn test_task_completion_metrics() {
        let subscriber = MetricsSubscriber::new();
        
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskCompleted {
                task_name: "test_task".to_string(),
                task_id: "test-1".to_string(),
                duration_ms: 1500,
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        let result = subscriber.handle_event(&event).await;
        assert!(result.is_ok());

        let summary = subscriber.get_metrics_summary().await;
        assert_eq!(summary.tasks_completed_success, 1);
        assert_eq!(summary.tasks_completed_failure, 0);
        assert!(summary.avg_task_duration_ms > 0.0);
    }

    #[tokio::test]
    async fn test_task_failure_metrics() {
        let subscriber = MetricsSubscriber::new();
        
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskFailed {
                task_name: "failing_task".to_string(),
                task_id: "fail-1".to_string(),
                error: "Test error".to_string(),
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        let result = subscriber.handle_event(&event).await;
        assert!(result.is_ok());

        let summary = subscriber.get_metrics_summary().await;
        assert_eq!(summary.tasks_completed_success, 0);
        assert_eq!(summary.tasks_completed_failure, 1);
    }

    #[tokio::test]
    async fn test_cache_metrics() {
        let subscriber = MetricsSubscriber::new();
        
        // Cache hit event
        let hit_event = EnhancedEvent {
            event: SystemEvent::Cache(CacheEvent::CacheHit {
                key: "test-key".to_string(),
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        // Cache miss event
        let miss_event = EnhancedEvent {
            event: SystemEvent::Cache(CacheEvent::CacheMiss {
                key: "test-key-2".to_string(),
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        subscriber.handle_event(&hit_event).await.unwrap();
        subscriber.handle_event(&miss_event).await.unwrap();

        let summary = subscriber.get_metrics_summary().await;
        assert_eq!(summary.cache_hits, 1);
        assert_eq!(summary.cache_misses, 1);
        assert_eq!(summary.cache_hit_ratio, 0.5);
    }

    #[tokio::test]
    async fn test_pipeline_metrics() {
        let subscriber = MetricsSubscriber::new();
        
        let event = EnhancedEvent {
            event: SystemEvent::Pipeline(PipelineEvent::PipelineCompleted {
                total_duration_ms: 5000,
                successful_tasks: 3,
                failed_tasks: 1,
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        let result = subscriber.handle_event(&event).await;
        assert!(result.is_ok());

        let summary = subscriber.get_metrics_summary().await;
        assert!(summary.events_total > 0);
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        let subscriber = MetricsSubscriber::new();
        
        // Add some metrics
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskCompleted {
                task_name: "test".to_string(),
                task_id: "test-1".to_string(),
                duration_ms: 1000,
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        subscriber.handle_event(&event).await.unwrap();
        
        let summary_before = subscriber.get_metrics_summary().await;
        assert!(summary_before.events_total > 0);

        // Reset metrics
        subscriber.reset_metrics().await;
        
        let summary_after = subscriber.get_metrics_summary().await;
        assert_eq!(summary_after.events_total, 0);
        assert_eq!(summary_after.tasks_completed_success, 0);
    }

    #[tokio::test]
    async fn test_multiple_task_durations() {
        let subscriber = MetricsSubscriber::new();
        
        // Add multiple completions for the same task
        for i in 0..3 {
            let event = EnhancedEvent {
                event: SystemEvent::Task(TaskEvent::TaskCompleted {
                    task_name: "repeated_task".to_string(),
                    task_id: format!("test-{}", i),
                    duration_ms: 1000 + (i as u64 * 500), // 1000, 1500, 2000 ms
                }),
                timestamp: SystemTime::now(),
                correlation_id: None,
                metadata: HashMap::new(),
            };
            subscriber.handle_event(&event).await.unwrap();
        }

        let summary = subscriber.get_metrics_summary().await;
        assert_eq!(summary.tasks_completed_success, 3);
        
        // Average should be (1000 + 1500 + 2000) / 3 = 1500
        assert!((summary.avg_task_duration_ms - 1500.0).abs() < 0.1);
    }
}