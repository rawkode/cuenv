//! Core types and enums for circuit breaker functionality.

use cuenv_core::Error;
use std::sync::Arc;
use std::time::Instant;

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed - requests pass through normally
    Closed,
    /// Circuit is open - requests fail immediately
    Open,
    /// Circuit is half-open - limited requests allowed to test recovery
    HalfOpen,
}

/// Which errors should trigger a retry
#[derive(Clone)]
pub enum RetryOn {
    /// Retry on all errors
    All,
    /// Retry only on network errors
    Network,
    /// Retry only on filesystem errors
    FileSystem,
    /// Retry on network and filesystem errors
    Transient,
    /// Custom retry predicate
    Custom(Arc<dyn Fn(&Error) -> bool + Send + Sync>),
}

impl std::fmt::Debug for RetryOn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetryOn::All => write!(f, "RetryOn::All"),
            RetryOn::Network => write!(f, "RetryOn::Network"),
            RetryOn::FileSystem => write!(f, "RetryOn::FileSystem"),
            RetryOn::Transient => write!(f, "RetryOn::Transient"),
            RetryOn::Custom(_) => write!(f, "RetryOn::Custom(<predicate>)"),
        }
    }
}

/// Statistics about circuit breaker state
#[derive(Debug)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub failure_count: usize,
    pub success_count: usize,
    pub half_open_calls: usize,
    pub last_failure_time: Option<Instant>,
    pub last_state_change: Instant,
}
