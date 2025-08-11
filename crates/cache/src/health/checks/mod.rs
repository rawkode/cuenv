//! Health check implementations
//!
//! Provides various health check handlers for Kubernetes probes,
//! basic health status, and detailed reporting.

pub mod metrics;

use crate::monitored::MonitoredCache;
use crate::security::audit::HealthStatus;
use crate::traits::Cache;
use hyper::{Body, Response, StatusCode};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{error, warn};

use super::reporting::HttpResponse;

/// Health check handlers
pub struct HealthChecks<C: Cache + Clone> {
    /// Monitored cache instance
    hardening: Arc<MonitoredCache<C>>,
    /// Health check timeout
    timeout: Duration,
}

impl<C: Cache + Clone + 'static> HealthChecks<C> {
    /// Create new health checks handler
    pub fn new(hardening: Arc<MonitoredCache<C>>, timeout: Duration) -> Self {
        Self { hardening, timeout }
    }

    /// Handle basic health check
    pub async fn basic_health(
        &self,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        match tokio::time::timeout(self.timeout, self.hardening.health_report()).await {
            Ok(Ok(report)) => {
                let status_code = match report.overall_status {
                    HealthStatus::Healthy => StatusCode::OK,
                    HealthStatus::Degraded => StatusCode::OK, // Still OK for basic check
                    HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
                    HealthStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
                };

                let response = serde_json::json!({
                    "status": report.overall_status.to_string(),
                    "uptime": report.uptime.as_secs(),
                    "timestamp": report.generated_at
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    "checks": report.summary.total_checks,
                    "healthy": report.summary.healthy_count,
                    "warnings": report.summary.warning_count,
                    "critical": report.summary.critical_count,
                    "down": report.summary.down_count
                });

                let json = serde_json::to_string(&response)?;
                let http_response = Response::builder()
                    .status(status_code)
                    .header("Content-Type", "application/json")
                    .header("Cache-Control", "no-cache, no-store, must-revalidate")
                    .body(Body::from(json))?;

                Ok(http_response)
            }
            Ok(Err(e)) => {
                error!("Health check failed: {}", e);
                let response = serde_json::json!({
                    "status": "ERROR",
                    "error": e.to_string(),
                    "timestamp": SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                });
                let json = serde_json::to_string(&response)?;
                let http_response = Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "application/json")
                    .body(Body::from(json))?;
                Ok(http_response)
            }
            Err(_) => {
                error!("Health check timed out");
                let response = serde_json::json!({
                    "status": "TIMEOUT",
                    "error": "Health check timed out",
                    "timestamp": SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                });
                let json = serde_json::to_string(&response)?;
                let http_response = Response::builder()
                    .status(StatusCode::REQUEST_TIMEOUT)
                    .header("Content-Type", "application/json")
                    .body(Body::from(json))?;
                Ok(http_response)
            }
        }
    }

    /// Handle detailed health report
    pub async fn detailed_health(
        &self,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        match tokio::time::timeout(self.timeout, self.hardening.health_report()).await {
            Ok(Ok(report)) => HttpResponse::ok_json(report),
            Ok(Err(e)) => {
                error!("Detailed health check failed: {}", e);
                Ok(HttpResponse::internal_error(format!(
                    "Health check failed: {e}"
                )))
            }
            Err(_) => {
                error!("Detailed health check timed out");
                Ok(HttpResponse::internal_error(
                    "Health check timed out".to_string(),
                ))
            }
        }
    }

    /// Handle Kubernetes readiness probe
    pub async fn readiness(
        &self,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        // Check if system is ready to serve traffic
        match self.hardening.health_report().await {
            Ok(report) => {
                let is_ready = matches!(
                    report.overall_status,
                    HealthStatus::Healthy | HealthStatus::Degraded
                );

                if is_ready {
                    HttpResponse::ok_text("Ready".to_string())
                } else {
                    let response = Response::builder()
                        .status(StatusCode::SERVICE_UNAVAILABLE)
                        .header("Content-Type", "text/plain")
                        .body(Body::from("Not Ready"))?;
                    Ok(response)
                }
            }
            Err(e) => {
                error!("Readiness check failed: {}", e);
                let response = Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .header("Content-Type", "text/plain")
                    .body(Body::from("Not Ready"))?;
                Ok(response)
            }
        }
    }

    /// Handle Kubernetes liveness probe
    pub async fn liveness(
        &self,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        // Check if the application is alive (even if not healthy)
        if self.hardening.is_shutting_down() {
            let response = Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .header("Content-Type", "text/plain")
                .body(Body::from("Shutting Down"))?;
            Ok(response)
        } else {
            HttpResponse::ok_text("Alive".to_string())
        }
    }

    /// Handle debug configuration endpoint
    pub async fn debug_config(
        &self,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        let validation_errors = self.hardening.get_config_validation_errors().await;
        HttpResponse::ok_json(serde_json::json!({
            "validation_errors": validation_errors
        }))
    }

    /// Handle debug shutdown endpoint
    pub async fn debug_shutdown(
        &self,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        warn!("Shutdown requested via debug endpoint");
        match self.hardening.shutdown().await {
            Ok(()) => HttpResponse::ok_text("Shutdown initiated".to_string()),
            Err(e) => Ok(HttpResponse::internal_error(format!(
                "Shutdown failed: {e}"
            ))),
        }
    }
}
