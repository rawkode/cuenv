//! Rate limiting for health endpoints
//!
//! Provides IP-based rate limiting to prevent abuse and ensure
//! fair resource usage across all clients.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Error type for rate limiting
#[derive(Debug, Clone)]
pub struct RateLimitError {
    pub message: String,
}

/// Rate limiting information per client
#[derive(Debug, Clone)]
struct ClientLimitInfo {
    /// Request count in current window
    count: u32,
    /// Window start time
    window_start: SystemTime,
}

/// Simple rate limiter for protecting endpoints
#[derive(Debug)]
pub struct RateLimiter {
    /// Request counts per client IP
    client_counts: HashMap<IpAddr, ClientLimitInfo>,
    /// Last cleanup time
    last_cleanup: SystemTime,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new() -> Self {
        Self {
            client_counts: HashMap::new(),
            last_cleanup: SystemTime::now(),
        }
    }

    /// Check if a client is within rate limits and update their count
    pub fn check_and_update(
        &mut self,
        client_ip: IpAddr,
        limit_per_minute: u32,
    ) -> Result<(), RateLimitError> {
        let now = SystemTime::now();

        // Clean up old entries every 5 minutes
        if now.duration_since(self.last_cleanup).unwrap_or_default() > Duration::from_secs(300) {
            self.cleanup_old_entries(now);
            self.last_cleanup = now;
        }

        let client_info = self
            .client_counts
            .entry(client_ip)
            .or_insert_with(|| ClientLimitInfo {
                count: 0,
                window_start: now,
            });

        // Reset window if it's been more than a minute
        if now
            .duration_since(client_info.window_start)
            .unwrap_or_default()
            >= Duration::from_secs(60)
        {
            client_info.count = 0;
            client_info.window_start = now;
        }

        // Check if within limit
        if client_info.count >= limit_per_minute {
            return Err(RateLimitError {
                message: format!("Rate limit exceeded for IP: {client_ip}"),
            });
        }

        // Increment count
        client_info.count += 1;
        Ok(())
    }

    /// Clean up old entries to prevent memory growth
    fn cleanup_old_entries(&mut self, now: SystemTime) {
        self.client_counts.retain(|_, info| {
            now.duration_since(info.window_start).unwrap_or_default() < Duration::from_secs(300)
        });
    }
}

/// Thread-safe rate limiter wrapper
pub struct RateLimiterManager {
    limiter: Arc<RwLock<RateLimiter>>,
    limit_per_minute: u32,
}

impl RateLimiterManager {
    /// Create a new rate limiter manager
    pub fn new(limit_per_minute: u32) -> Self {
        Self {
            limiter: Arc::new(RwLock::new(RateLimiter::new())),
            limit_per_minute,
        }
    }

    /// Check if a client is within rate limits
    pub async fn check_rate_limit(&self, client_ip: IpAddr) -> Result<(), ()> {
        let mut limiter = self.limiter.write().await;
        limiter
            .check_and_update(client_ip, self.limit_per_minute)
            .map_err(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter() {
        let mut limiter = RateLimiter::new();
        let client_ip = "127.0.0.1".parse().unwrap();

        // Should allow requests up to limit
        for _ in 0..5 {
            assert!(limiter.check_and_update(client_ip, 5).is_ok());
        }

        // Should reject over limit
        assert!(limiter.check_and_update(client_ip, 5).is_err());
    }

    #[tokio::test]
    async fn test_rate_limiter_manager() {
        let manager = RateLimiterManager::new(3);
        let client_ip = "192.168.1.1".parse().unwrap();

        // Should allow requests up to limit
        for _ in 0..3 {
            assert!(manager.check_rate_limit(client_ip).await.is_ok());
        }

        // Should reject over limit
        assert!(manager.check_rate_limit(client_ip).await.is_err());
    }
}
