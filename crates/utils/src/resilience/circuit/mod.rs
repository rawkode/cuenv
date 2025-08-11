//! Resilience patterns for error recovery including retry logic and circuit breakers
//!
//! This module provides reusable utilities for handling transient failures
//! and protecting against cascading failures in external operations.
//!
//! ## Architecture
//!
//! This module is organized into focused sub-modules:
//!
//! - [`types`] - Core types and enums (CircuitState, RetryOn, etc.)
//! - [`config`] - Configuration structs for retry and circuit breaker behavior
//! - [`metrics`] - Metrics and statistics tracking
//! - [`transitions`] - State transition logic for circuit breakers
//! - [`state`] - Main circuit breaker state management and execution
//! - [`retry`] - Retry logic and recovery suggestions
//! - [`tests`] - Integration tests
//!
//! ## Examples
//!
//! ### Basic Retry
//!
//! ```rust,no_run
//! use cuenv_utils::resilience::circuit::{retry, RetryConfig};
//!
//! # async fn example() -> Result<String, cuenv_core::Error> {
//! let config = RetryConfig::for_network();
//! let result = retry(&config, || async {
//!     // Your operation here
//!     Ok("success".to_string())
//! }).await;
//! result
//! # }
//! ```
//!
//! ### Circuit Breaker
//!
//! ```rust,no_run
//! use cuenv_utils::resilience::circuit::{CircuitBreaker, CircuitBreakerConfig};
//!
//! # async fn example() -> Result<String, cuenv_core::Error> {
//! let config = CircuitBreakerConfig::default();
//! let cb = CircuitBreaker::new(config);
//!
//! let result = cb.call(|| async {
//!     // Your operation here
//!     Ok("success".to_string())
//! }).await;
//! result
//! # }
//! ```
//!
//! ### Combined Retry with Circuit Breaker
//!
//! ```rust,no_run
//! use cuenv_utils::resilience::circuit::{retry_with_circuit_breaker, RetryConfig, CircuitBreaker, CircuitBreakerConfig};
//!
//! # async fn example() -> Result<String, cuenv_core::Error> {
//! let retry_config = RetryConfig::for_network();
//! let cb_config = CircuitBreakerConfig::default();
//! let cb = CircuitBreaker::new(cb_config);
//!
//! let result = retry_with_circuit_breaker(&retry_config, &cb, || async {
//!     // Your operation here
//!     Ok("success".to_string())
//! }).await;
//! result
//! # }
//! ```

pub mod config;
pub mod metrics;
pub mod retry;
pub mod state;
#[cfg(test)]
pub mod tests;
pub mod transitions;
pub mod types;

// Re-export public API
pub use config::{CircuitBreakerConfig, RetryConfig};
pub use retry::{retry, retry_with_circuit_breaker, suggest_recovery};
pub use state::CircuitBreaker;
pub use types::{CircuitBreakerStats, CircuitState, RetryOn};
