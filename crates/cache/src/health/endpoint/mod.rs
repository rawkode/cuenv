//! Main health endpoint server
//!
//! Coordinates the HTTP server that handles health check requests,
//! metrics, and operational endpoints.

mod router;

use crate::monitored::MonitoredCache;
use crate::traits::Cache;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{error, info};

use super::auth::AuthManager;
use super::checks::HealthChecks;
use super::config::HealthEndpointConfig;
use super::limiting::RateLimiterManager;

pub use router::RequestRouter;

/// HTTP health check server
pub struct HealthEndpoint<C: Cache + Clone> {
    /// Reliability and monitoring wrapper
    hardening: Arc<MonitoredCache<C>>,
    /// Server bind address
    bind_addr: SocketAddr,
    /// Authentication manager
    auth_manager: AuthManager,
    /// Request rate limiter
    rate_limiter: RateLimiterManager,
    /// Server configuration
    config: HealthEndpointConfig,
    /// Health check handlers
    health_checks: Arc<HealthChecks<C>>,
}

impl<C: Cache + Clone + 'static> HealthEndpoint<C> {
    /// Create a new health endpoint server
    pub fn new(
        hardening: Arc<MonitoredCache<C>>,
        bind_addr: SocketAddr,
        config: HealthEndpointConfig,
    ) -> Self {
        let auth_manager = AuthManager::new(config.require_auth);
        let rate_limiter = RateLimiterManager::new(config.rate_limit_per_minute);
        let health_checks = Arc::new(HealthChecks::new(
            Arc::clone(&hardening),
            config.health_check_timeout,
        ));

        Self {
            hardening,
            bind_addr,
            auth_manager,
            rate_limiter,
            config,
            health_checks,
        }
    }

    /// Add an authentication token
    pub async fn add_auth_token(
        &self,
        token: String,
        name: String,
        expires_at: Option<SystemTime>,
        allowed_endpoints: Vec<String>,
    ) {
        self.auth_manager
            .add_token(token, name, expires_at, allowed_endpoints)
            .await;
    }

    /// Start the health endpoint server
    pub async fn serve(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let bind_addr = self.bind_addr;
        let enable_metrics = self.config.enable_metrics;
        let enable_debug = self.config.enable_debug;

        info!("Starting health endpoint server on {}", bind_addr);

        let make_svc = make_service_fn(move |conn: &hyper::server::conn::AddrStream| {
            let endpoint = Arc::clone(&self);
            let remote_addr = conn.remote_addr();

            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    let endpoint = Arc::clone(&endpoint);
                    async move { endpoint.handle_request(req, remote_addr.ip()).await }
                }))
            }
        });

        let server = Server::bind(&bind_addr).serve(make_svc);

        info!("Health endpoint server ready on http://{}", bind_addr);
        info!("Available endpoints:");
        info!("  GET /health - Basic health check");
        info!("  GET /health/detailed - Detailed health report");
        info!("  GET /health/ready - Readiness probe");
        info!("  GET /health/live - Liveness probe");
        if enable_metrics {
            info!("  GET /metrics - Prometheus metrics");
        }
        if enable_debug {
            info!("  GET /debug/* - Debug endpoints (disabled in production)");
        }

        match server.await {
            Ok(()) => Ok(()),
            Err(e) => {
                error!("Health endpoint server error: {}", e);
                Err(Box::new(e))
            }
        }
    }

    /// Handle incoming HTTP requests
    async fn handle_request(
        &self,
        req: Request<Body>,
        client_ip: std::net::IpAddr,
    ) -> Result<Response<Body>, Infallible> {
        let router = RequestRouter::new(
            Arc::clone(&self.hardening),
            &self.auth_manager,
            &self.rate_limiter,
            &self.config,
            Arc::clone(&self.health_checks),
        );

        Ok(router.route(req, client_ip).await)
    }
}
