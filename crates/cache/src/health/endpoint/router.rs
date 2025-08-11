//! Request routing for health endpoints
//!
//! Routes incoming HTTP requests to appropriate handlers based on
//! path and method, with authentication and rate limiting checks.

use crate::monitored::MonitoredCache;
use crate::traits::Cache;
use hyper::{Body, Method, Request, Response};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::health::auth::AuthManager;
use crate::health::checks::{metrics::generate_metrics, HealthChecks};
use crate::health::config::HealthEndpointConfig;
use crate::health::limiting::RateLimiterManager;
use crate::health::reporting::HttpResponse;

/// Request router for health endpoints
pub struct RequestRouter<'a, C: Cache + Clone> {
    hardening: Arc<MonitoredCache<C>>,
    auth_manager: &'a AuthManager,
    rate_limiter: &'a RateLimiterManager,
    config: &'a HealthEndpointConfig,
    health_checks: Arc<HealthChecks<C>>,
}

impl<'a, C: Cache + Clone + 'static> RequestRouter<'a, C> {
    /// Create a new request router
    pub fn new(
        hardening: Arc<MonitoredCache<C>>,
        auth_manager: &'a AuthManager,
        rate_limiter: &'a RateLimiterManager,
        config: &'a HealthEndpointConfig,
        health_checks: Arc<HealthChecks<C>>,
    ) -> Self {
        Self {
            hardening,
            auth_manager,
            rate_limiter,
            config,
            health_checks,
        }
    }

    /// Route a request to the appropriate handler
    pub async fn route(&self, req: Request<Body>, client_ip: IpAddr) -> Response<Body> {
        let start_time = std::time::Instant::now();
        let method = req.method().clone();
        let path = req.uri().path().to_string();

        debug!("Handling {} {} from {}", method, path, client_ip);

        // Check rate limiting
        match self.rate_limiter.check_rate_limit(client_ip).await {
            Ok(()) => {}
            Err(_) => {
                warn!("Rate limit exceeded for {}", client_ip);
                return HttpResponse::rate_limited();
            }
        }

        let response = match (&method, path.as_str()) {
            (&Method::GET, "/health") => self.health_checks.basic_health().await,
            (&Method::GET, "/health/detailed") => self.health_checks.detailed_health().await,
            (&Method::GET, "/health/ready") => self.health_checks.readiness().await,
            (&Method::GET, "/health/live") => self.health_checks.liveness().await,
            (&Method::GET, "/metrics") if self.config.enable_metrics => {
                self.handle_metrics(&req).await
            }
            (&Method::GET, path) if path.starts_with("/debug/") && self.config.enable_debug => {
                self.handle_debug(&req, path).await
            }
            (&Method::GET, "/") => self.handle_index().await,
            _ => Ok(HttpResponse::not_found()),
        };

        let duration = start_time.elapsed();
        let status = match &response {
            Ok(resp) => resp.status().as_u16(),
            Err(_) => 500,
        };

        debug!(
            "Handled {} {} from {} -> {} in {:?}",
            method, path, client_ip, status, duration
        );

        match response {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!("Error handling request: {}", e);
                HttpResponse::internal_error(format!("Internal error: {e}"))
            }
        }
    }

    /// Handle metrics endpoint
    async fn handle_metrics(
        &self,
        req: &Request<Body>,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        match self.auth_manager.validate(req, "/metrics").await {
            Ok(()) => {}
            Err(()) => return Ok(HttpResponse::unauthorized()),
        }

        generate_metrics(&self.hardening).await
    }

    /// Handle debug endpoints
    async fn handle_debug(
        &self,
        req: &Request<Body>,
        path: &str,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        match self.auth_manager.validate(req, path).await {
            Ok(()) => {}
            Err(()) => return Ok(HttpResponse::unauthorized()),
        }

        match path {
            "/debug/config" => self.health_checks.debug_config().await,
            "/debug/shutdown" => self.health_checks.debug_shutdown().await,
            _ => Ok(HttpResponse::not_found()),
        }
    }

    /// Handle index page
    async fn handle_index(
        &self,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        let html = self.generate_index_html();
        HttpResponse::ok_html(html)
    }

    /// Generate index HTML
    fn generate_index_html(&self) -> String {
        let mut html = r#"<!DOCTYPE html>
<html>
<head>
    <title>cuenv Cache Health Monitor</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .healthy { color: green; }
        .warning { color: orange; }
        .critical { color: red; }
        .down { color: darkred; font-weight: bold; }
        ul { list-style-type: none; padding: 0; }
        li { margin: 10px 0; }
        a { text-decoration: none; color: #0066cc; }
        a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <h1>cuenv Cache Health Monitor</h1>
    <p>Production health monitoring endpoints for the cuenv cache system.</p>
    
    <h2>Available Endpoints</h2>
    <ul>
        <li><a href="/health">GET /health</a> - Basic health check (JSON)</li>
        <li><a href="/health/detailed">GET /health/detailed</a> - Detailed health report (JSON)</li>
        <li><a href="/health/ready">GET /health/ready</a> - Kubernetes readiness probe</li>
        <li><a href="/health/live">GET /health/live</a> - Kubernetes liveness probe</li>"#
            .to_string();

        if self.config.enable_metrics {
            html.push_str(
                r#"
        <li><a href="/metrics">GET /metrics</a> - Prometheus metrics</li>"#,
            );
        }

        if self.config.enable_debug {
            html.push_str(r#"
        <li><strong>Debug Endpoints (Production: Disabled)</strong></li>
        <li>&nbsp;&nbsp;<a href="/debug/config">GET /debug/config</a> - Configuration validation</li>"#);
        }

        html.push_str(
            r#"
    </ul>
    
    <h2>Authentication</h2>
    <p>Sensitive endpoints require Bearer token authentication when enabled.</p>
    
    <h2>Rate Limiting</h2>
    <p>Requests are limited to prevent abuse. Current limit: 60 requests per minute per IP.</p>
</body>
</html>"#,
        );

        html
    }
}
