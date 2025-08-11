//! Monitoring and observability for the cache system
//!
//! This module provides comprehensive monitoring capabilities including:
//! - Prometheus metrics export
//! - OpenTelemetry distributed tracing
//! - Cache hit rate analysis
//! - Performance flamegraphs
//! - Real-time dashboards

mod analyzer;
#[path = "metrics/mod.rs"]
mod metrics;
mod monitor;
#[path = "profiler/mod.rs"]
mod profiler;
#[path = "stats/mod.rs"]
mod stats;
mod traced;
mod types;

// Re-export public types
pub use analyzer::{HitRateReport, OperationStats, PatternStats};
pub use monitor::CacheMonitor;
pub use stats::RealTimeStatsReport;
pub use traced::TracedOperation;

// Re-export CacheStatistics from traits module
pub use crate::traits::CacheStatistics;
