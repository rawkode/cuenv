//! HTTP health check endpoint for production monitoring
//!
//! This module provides a production-ready HTTP server for health checks,
//! metrics, and operational endpoints. It integrates with monitoring systems
//! like Prometheus, Grafana, and Kubernetes health probes.
//!
//! Design principles:
//! - No `?` operator - explicit error handling only
//! - Production-grade HTTP server with proper error handling
//! - Comprehensive health checks and metrics exposure
//! - Security considerations for operational endpoints

pub mod auth;
pub mod checks;
pub mod config;
pub mod endpoint;
pub mod limiting;
pub mod reporting;

// Re-export main types
pub use config::HealthEndpointConfig;
pub use endpoint::HealthEndpoint;
