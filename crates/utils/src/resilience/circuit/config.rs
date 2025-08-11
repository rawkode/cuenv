//! Configuration structs and implementations for retry and circuit breaker behavior.

use super::types::RetryOn;
use cuenv_core::Error;
use std::time::{Duration, SystemTime};

/// Default maximum number of retry attempts
const DEFAULT_MAX_RETRIES: usize = 3;

/// Default base delay for exponential backoff (100ms)
const DEFAULT_BASE_DELAY: Duration = Duration::from_millis(100);

/// Default maximum delay for exponential backoff (10s)
const DEFAULT_MAX_DELAY: Duration = Duration::from_secs(10);

/// Default jitter factor (0.1 = 10% randomization)
const DEFAULT_JITTER_FACTOR: f64 = 0.1;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: usize,
    /// Base delay for exponential backoff
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Jitter factor for randomization (0.0 to 1.0)
    pub jitter_factor: f64,
    /// Whether to retry on specific error types
    pub retry_on: RetryOn,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay: DEFAULT_BASE_DELAY,
            max_delay: DEFAULT_MAX_DELAY,
            jitter_factor: DEFAULT_JITTER_FACTOR,
            retry_on: RetryOn::Transient,
        }
    }
}

impl RetryConfig {
    /// Create a retry config for network operations
    pub fn for_network() -> Self {
        Self {
            max_retries: 5,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(30),
            jitter_factor: 0.2,
            retry_on: RetryOn::Network,
        }
    }

    /// Create a retry config for filesystem operations
    pub fn for_filesystem() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(2),
            jitter_factor: 0.1,
            retry_on: RetryOn::FileSystem,
        }
    }

    /// Check if an error should be retried
    pub fn should_retry(&self, error: &Error) -> bool {
        match &self.retry_on {
            RetryOn::All => true,
            RetryOn::Network => matches!(error, Error::Network { .. }),
            RetryOn::FileSystem => matches!(error, Error::FileSystem { .. }),
            RetryOn::Transient => {
                matches!(error, Error::Network { .. } | Error::FileSystem { .. })
            }
            RetryOn::Custom(predicate) => predicate(error),
        }
    }

    /// Calculate delay for a given attempt with exponential backoff and jitter
    pub fn calculate_delay(&self, attempt: usize) -> Duration {
        let exponential_delay = self.base_delay * 2u32.pow(attempt as u32);
        let capped_delay = exponential_delay.min(self.max_delay);

        // Add jitter to prevent thundering herd
        if self.jitter_factor > 0.0 {
            let jitter_range = capped_delay.as_millis() as f64 * self.jitter_factor;
            // Simple pseudo-random jitter based on system time
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let seed = now.as_nanos() as f64;
            let normalized = ((seed % 1000.0) / 1000.0 - 0.5) * 2.0;
            let jitter = normalized * jitter_range;
            let final_millis = (capped_delay.as_millis() as f64 + jitter).max(0.0) as u64;
            Duration::from_millis(final_millis)
        } else {
            capped_delay
        }
    }
}

/// Configuration for circuit breaker behavior
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: usize,
    /// Success threshold to close the circuit from half-open state
    pub success_threshold: usize,
    /// Time window for counting failures
    pub timeout: Duration,
    /// Duration to wait before attempting half-open state
    pub break_duration: Duration,
    /// Maximum number of requests in half-open state
    pub half_open_max_calls: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            break_duration: Duration::from_secs(30),
            half_open_max_calls: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_calculate_delay_with_jitter() {
        let config = RetryConfig {
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
            jitter_factor: 0.5,
            ..Default::default()
        };

        // Test multiple times to ensure jitter is working
        let mut delays = Vec::new();
        for _ in 0..10 {
            delays.push(config.calculate_delay(2));
        }

        // All delays should be different due to jitter
        let unique_delays: HashSet<_> = delays.iter().collect();
        assert!(unique_delays.len() > 1);

        // All should be within expected range (400ms ± 50%)
        for delay in delays {
            assert!(delay >= Duration::from_millis(200));
            assert!(delay <= Duration::from_millis(600));
        }
    }
}
