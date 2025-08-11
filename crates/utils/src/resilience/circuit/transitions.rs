//! State transition logic for circuit breaker.

use super::config::CircuitBreakerConfig;
use super::metrics::MetricsState;
use super::types::CircuitState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

/// Handles state transitions for circuit breaker
pub struct StateTransitions {
    config: CircuitBreakerConfig,
    metrics: Arc<MetricsState>,
}

impl StateTransitions {
    /// Create new state transitions handler
    pub fn new(config: CircuitBreakerConfig, metrics: Arc<MetricsState>) -> Self {
        Self { config, metrics }
    }

    /// Transition to open state
    pub async fn transition_to_open(&self) {
        let mut state = self.metrics.state.write().await;
        if *state != CircuitState::Open {
            log::warn!("Circuit breaker opening");
            *state = CircuitState::Open;
            *self.metrics.last_state_change.lock().await = Instant::now();
            self.metrics.increment_generation();
            self.metrics.reset_counters();
        }
    }

    /// Transition to half-open state
    pub async fn transition_to_half_open(&self) {
        let mut state = self.metrics.state.write().await;
        if *state != CircuitState::HalfOpen {
            log::info!("Circuit breaker entering half-open state");
            *state = CircuitState::HalfOpen;
            *self.metrics.last_state_change.lock().await = Instant::now();
            self.metrics.increment_generation();
            self.metrics.reset_counters();
        }
    }

    /// Transition to closed state
    pub async fn transition_to_closed(&self) {
        let mut state = self.metrics.state.write().await;
        if *state != CircuitState::Closed {
            log::info!("Circuit breaker closing");
            *state = CircuitState::Closed;
            *self.metrics.last_state_change.lock().await = Instant::now();
            self.metrics.increment_generation();
            self.metrics.reset_counters();
        }
    }

    /// Record a successful call and handle state transitions
    pub async fn record_success(&self, generation: u64) {
        // Only record if we're still in the same generation
        if generation != self.metrics.current_generation() {
            return;
        }

        let state = *self.metrics.state.read().await;

        match state {
            CircuitState::HalfOpen => {
                let count = self.metrics.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.success_threshold {
                    self.transition_to_closed().await;
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                self.metrics.failure_count.store(0, Ordering::SeqCst);
                *self.metrics.last_failure_time.lock().await = None;
            }
            CircuitState::Open => {} // Shouldn't happen
        }
    }

    /// Record a failed call and handle state transitions
    pub async fn record_failure(&self, generation: u64) {
        // Only record if we're still in the same generation
        if generation != self.metrics.current_generation() {
            return;
        }

        let now = Instant::now();
        let mut last_failure = self.metrics.last_failure_time.lock().await;

        // Reset failure count if outside the timeout window
        if let Some(last) = *last_failure {
            if now.duration_since(last) > self.config.timeout {
                self.metrics.failure_count.store(0, Ordering::SeqCst);
            }
        }

        *last_failure = Some(now);
        drop(last_failure);

        let state = *self.metrics.state.read().await;

        match state {
            CircuitState::Closed => {
                let count = self.metrics.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.failure_threshold {
                    self.transition_to_open().await;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                self.transition_to_open().await;
            }
            CircuitState::Open => {} // Already open
        }
    }

    /// Check if we should transition from Open to HalfOpen based on break duration
    pub async fn check_half_open_transition(&self) -> bool {
        let state = *self.metrics.state.read().await;
        if state == CircuitState::Open {
            let last_change = *self.metrics.last_state_change.lock().await;
            if last_change.elapsed() >= self.config.break_duration {
                self.transition_to_half_open().await;
                return true;
            }
        }
        false
    }
}
