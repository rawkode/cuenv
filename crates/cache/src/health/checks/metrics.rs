//! Prometheus metrics generation
//!
//! Converts health reports into Prometheus-compatible metrics format.

use crate::monitored::MonitoredCache;
use crate::security::audit::HealthStatus;
use crate::traits::Cache;
use hyper::{Body, Response};
use std::sync::Arc;
use tracing::error;

use crate::health::reporting::HttpResponse;

/// Generate Prometheus metrics from health report
pub async fn generate_metrics<C: Cache + Clone>(
    hardening: &Arc<MonitoredCache<C>>,
) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
    match hardening.health_report().await {
        Ok(report) => {
            let mut metrics = Vec::new();

            // System uptime
            metrics.push(format!(
                "# HELP cuenv_uptime_seconds System uptime in seconds\n# TYPE cuenv_uptime_seconds gauge\ncuenv_uptime_seconds {}",
                report.uptime.as_secs()
            ));

            // Overall health status (0=healthy, 1=warning, 2=critical, 3=down)
            let status_value = match report.overall_status {
                HealthStatus::Healthy => 0,
                HealthStatus::Degraded => 1,
                HealthStatus::Unhealthy => 2,
                HealthStatus::Unknown => 3,
            };
            metrics.push(format!(
                "# HELP cuenv_health_status Overall system health status\n# TYPE cuenv_health_status gauge\ncuenv_health_status {status_value}"
            ));

            // Component health counts
            metrics.push(format!(
                "# HELP cuenv_health_components_total Total number of health check components\n# TYPE cuenv_health_components_total gauge\ncuenv_health_components_total {}",
                report.summary.total_checks
            ));

            metrics.push(format!(
                "# HELP cuenv_health_components_healthy Number of healthy components\n# TYPE cuenv_health_components_healthy gauge\ncuenv_health_components_healthy {}",
                report.summary.healthy_count
            ));

            metrics.push(format!(
                "# HELP cuenv_health_components_warning Number of components with warnings\n# TYPE cuenv_health_components_warning gauge\ncuenv_health_components_warning {}",
                report.summary.warning_count
            ));

            metrics.push(format!(
                "# HELP cuenv_health_components_critical Number of critical components\n# TYPE cuenv_health_components_critical gauge\ncuenv_health_components_critical {}",
                report.summary.critical_count
            ));

            metrics.push(format!(
                "# HELP cuenv_health_components_down Number of down components\n# TYPE cuenv_health_components_down gauge\ncuenv_health_components_down {}",
                report.summary.down_count
            ));

            // Individual component metrics
            for component in &report.components {
                let component_status = match component.status {
                    HealthStatus::Healthy => 0,
                    HealthStatus::Degraded => 1,
                    HealthStatus::Unhealthy => 2,
                    HealthStatus::Unknown => 3,
                };

                metrics.push(format!(
                    "cuenv_component_health_status{{component=\"{}\"}} {}",
                    component.component, component_status
                ));

                metrics.push(format!(
                    "cuenv_component_check_duration_seconds{{component=\"{}\"}} {}",
                    component.component,
                    component.check_duration.as_secs_f64()
                ));
            }

            let metrics_text = metrics.join("\n");
            HttpResponse::ok_text(metrics_text)
        }
        Err(e) => {
            error!("Failed to get metrics: {}", e);
            Ok(HttpResponse::internal_error(format!(
                "Failed to get metrics: {e}"
            )))
        }
    }
}
