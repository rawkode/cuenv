//! Circuit breaker state management and execution logic.

use super::config::CircuitBreakerConfig;
use super::metrics::MetricsState;
use super::transitions::StateTransitions;
use super::types::{CircuitBreakerStats, CircuitState};
use cuenv_core::{Error, Result};
use std::future::Future;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    metrics: Arc<MetricsState>,
    transitions: StateTransitions,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        let metrics = Arc::new(MetricsState::new());
        let transitions = StateTransitions::new(config.clone(), Arc::clone(&metrics));

        Self {
            config: config.clone(),
            metrics,
            transitions,
        }
    }

    /// Get the current state of the circuit
    pub async fn state(&self) -> CircuitState {
        let state = *self.metrics.state.read().await;

        // Check if we should transition from Open to HalfOpen
        if state == CircuitState::Open && self.transitions.check_half_open_transition().await {
            return CircuitState::HalfOpen;
        }

        state
    }

    /// Execute an operation through the circuit breaker
    pub async fn call<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let current_state = self.state().await;

        match current_state {
            CircuitState::Open => Err(Error::configuration(
                "Circuit breaker is open - service unavailable",
            )),
            CircuitState::HalfOpen => {
                let calls = self.metrics.half_open_calls.fetch_add(1, Ordering::SeqCst);
                if calls >= self.config.half_open_max_calls {
                    return Err(Error::configuration(
                        "Circuit breaker half-open limit reached",
                    ));
                }
                self.execute_with_recording(operation).await
            }
            CircuitState::Closed => self.execute_with_recording(operation).await,
        }
    }

    /// Execute operation and record the result
    async fn execute_with_recording<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let generation = self.metrics.current_generation();
        let result = operation().await;

        match result {
            Ok(_) => {
                self.transitions.record_success(generation).await;
            }
            Err(_) => {
                self.transitions.record_failure(generation).await;
            }
        }

        result
    }

    /// Get current circuit breaker statistics
    pub async fn stats(&self) -> CircuitBreakerStats {
        self.metrics.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Fail 3 times
        for _ in 0..3 {
            let _: Result<()> = cb
                .call(|| async { Err(Error::network("test", "fail")) })
                .await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);

        // Next call should fail immediately
        let result = cb.call(|| async { Ok("should not execute") }).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circuit breaker is open"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            break_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..2 {
            let _: Result<()> = cb
                .call(|| async { Err(Error::network("test", "fail")) })
                .await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for break duration
        sleep(Duration::from_millis(150)).await;

        // Should be half-open now
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // Two successful calls should close it
        for _ in 0..2 {
            let _ = cb.call(|| async { Ok("success") }).await;
        }

        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            break_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..2 {
            let _: Result<()> = cb
                .call(|| async { Err(Error::network("test", "fail")) })
                .await;
        }

        // Wait for break duration
        sleep(Duration::from_millis(150)).await;

        // Should be half-open
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // Failure in half-open should reopen
        let _: Result<()> = cb
            .call(|| async { Err(Error::network("test", "fail")) })
            .await;

        assert_eq!(cb.state().await, CircuitState::Open);
    }
}
