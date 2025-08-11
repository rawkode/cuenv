//! Resilience patterns like circuit breakers.
//!
//! This module provides mechanisms to build robust, fault-tolerant systems
//! that can gracefully handle and recover from transient failures.
//!
//! ## Key Components
//!
//! - **`circuit`**: Implements the circuit breaker pattern to prevent
//!   repeatedly calling a service that is known to be failing.

pub mod circuit;

pub use circuit::{
    retry, retry_with_circuit_breaker, suggest_recovery, CircuitBreaker, CircuitBreakerConfig,
    CircuitBreakerStats, CircuitState, RetryConfig, RetryOn,
};
