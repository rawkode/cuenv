//! Traced operations for distributed tracing support

use crate::errors::CacheError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tracing::Span;

use super::monitor::CacheMonitor;

/// Traced operation handle
pub struct TracedOperation {
    span: Span,
    start_time: Instant,
    monitor: CacheMonitor,
    completed: AtomicBool,
}

impl TracedOperation {
    pub(super) fn new(operation: &str, key: &str, monitor: CacheMonitor) -> Self {
        let span = tracing::info_span!("cache_operation", operation = operation, key = key);

        Self {
            span,
            start_time: Instant::now(),
            monitor,
            completed: AtomicBool::new(false),
        }
    }

    /// Complete the operation successfully
    pub fn complete(self) {
        if !self.completed.swap(true, Ordering::Relaxed) {
            tracing::info!(parent: &self.span, "Operation completed successfully");

            let _duration = self.start_time.elapsed();
            self.monitor.decrement_in_flight();
        }
    }

    /// Complete the operation with an error
    pub fn error(self, error: &CacheError) {
        if !self.completed.swap(true, Ordering::Relaxed) {
            tracing::error!(parent: &self.span, error = %error, "Operation failed");
            self.monitor.decrement_in_flight();
        }
    }
}

impl Drop for TracedOperation {
    fn drop(&mut self) {
        // Ensure we decrement the counter even if the operation wasn't properly completed
        if !self.completed.swap(true, Ordering::Relaxed) {
            self.monitor.decrement_in_flight();
        }
    }
}
