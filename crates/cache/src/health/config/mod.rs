//! Health endpoint configuration
//!
//! Provides configuration for the health check HTTP server including
//! authentication, rate limiting, timeouts, and feature toggles.

use std::time::Duration;

/// Health endpoint configuration
#[derive(Debug, Clone)]
pub struct HealthEndpointConfig {
    /// Enable authentication for sensitive endpoints
    pub require_auth: bool,
    /// Rate limit requests per minute per IP
    pub rate_limit_per_minute: u32,
    /// Timeout for health checks
    pub health_check_timeout: Duration,
    /// Enable detailed metrics
    pub enable_metrics: bool,
    /// Enable debug endpoints (should be false in production)
    pub enable_debug: bool,
}

impl Default for HealthEndpointConfig {
    fn default() -> Self {
        Self {
            require_auth: true,
            rate_limit_per_minute: 60,
            health_check_timeout: Duration::from_secs(10),
            enable_metrics: true,
            enable_debug: false,
        }
    }
}
