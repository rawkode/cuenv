//! Metrics and statistics tracking for circuit breaker.

use super::types::{CircuitBreakerStats, CircuitState};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

/// Internal state tracking for circuit breaker metrics
#[derive(Debug)]
pub struct MetricsState {
    pub state: Arc<RwLock<CircuitState>>,
    pub failure_count: Arc<AtomicUsize>,
    pub success_count: Arc<AtomicUsize>,
    pub half_open_calls: Arc<AtomicUsize>,
    pub last_failure_time: Arc<Mutex<Option<Instant>>>,
    pub last_state_change: Arc<Mutex<Instant>>,
    pub generation: Arc<AtomicU64>,
}

impl MetricsState {
    /// Create new metrics state
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(AtomicUsize::new(0)),
            success_count: Arc::new(AtomicUsize::new(0)),
            half_open_calls: Arc::new(AtomicUsize::new(0)),
            last_failure_time: Arc::new(Mutex::new(None)),
            last_state_change: Arc::new(Mutex::new(Instant::now())),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Reset internal counters
    pub fn reset_counters(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_calls.store(0, Ordering::SeqCst);
    }

    /// Get current circuit breaker statistics
    pub async fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            state: *self.state.read().await,
            failure_count: self.failure_count.load(Ordering::SeqCst),
            success_count: self.success_count.load(Ordering::SeqCst),
            half_open_calls: self.half_open_calls.load(Ordering::SeqCst),
            last_failure_time: *self.last_failure_time.lock().await,
            last_state_change: *self.last_state_change.lock().await,
        }
    }

    /// Increment generation counter for state changes
    pub fn increment_generation(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Get current generation
    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

impl Default for MetricsState {
    fn default() -> Self {
        Self::new()
    }
}
