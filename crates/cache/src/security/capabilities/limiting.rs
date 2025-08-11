//! Rate limiting and throttling functionality

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Rate limiting state
#[derive(Debug)]
pub struct RateLimitState {
    pub last_operation: SystemTime,
    pub operation_count: u64,
    pub window_start: SystemTime,
}

/// Rate limiter for tracking operation limits
#[derive(Debug)]
pub struct RateLimiter {
    /// Rate limit states per token
    states: HashMap<String, RateLimitState>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Check if an operation is allowed under the rate limit
    pub fn check_rate_limit(&mut self, token_id: &str, rate_limit: f64) -> bool {
        let now = SystemTime::now();
        let rate_state =
            self.states
                .entry(token_id.to_string())
                .or_insert_with(|| RateLimitState {
                    last_operation: now,
                    operation_count: 0,
                    window_start: now,
                });

        let window_duration = Duration::from_secs(1); // 1-second window
        let max_operations = rate_limit as u64;

        // Reset window if needed
        if now
            .duration_since(rate_state.window_start)
            .unwrap_or_default()
            >= window_duration
        {
            rate_state.window_start = now;
            rate_state.operation_count = 0;
        }

        // Check if under limit
        if rate_state.operation_count < max_operations {
            rate_state.operation_count += 1;
            rate_state.last_operation = now;
            true
        } else {
            false
        }
    }

    /// Clear rate limit state for a token
    pub fn clear_token_state(&mut self, token_id: &str) {
        self.states.remove(token_id);
    }

    /// Clear all rate limit states
    pub fn clear_all(&mut self) {
        self.states.clear();
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}
