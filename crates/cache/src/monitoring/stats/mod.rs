//! Real-time statistics collection

use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Real-time statistics collector
#[allow(dead_code)]
pub struct RealTimeStats {
    /// Current operations in flight
    operations_in_flight: AtomicU64,
    /// Peak operations per second
    peak_ops_per_second: AtomicU64,
    /// Average response time (microseconds)
    avg_response_time_us: AtomicU64,
    /// P99 response time (microseconds)
    p99_response_time_us: AtomicU64,
    /// Response time samples
    response_times: RwLock<Vec<u64>>,
}

impl RealTimeStats {
    pub fn new() -> Self {
        Self {
            operations_in_flight: AtomicU64::new(0),
            peak_ops_per_second: AtomicU64::new(0),
            avg_response_time_us: AtomicU64::new(0),
            p99_response_time_us: AtomicU64::new(0),
            response_times: RwLock::new(Vec::with_capacity(10000)),
        }
    }

    pub fn record_operation(&self, duration: Duration) {
        let duration_us = duration.as_micros() as u64;

        // Update response times
        {
            let mut times = self.response_times.write();
            times.push(duration_us);

            // Keep only recent samples
            if times.len() > 10000 {
                times.drain(0..5000);
            }

            // Calculate statistics
            if !times.is_empty() {
                let sum: u64 = times.iter().sum();
                let avg = sum / times.len() as u64;
                self.avg_response_time_us.store(avg, Ordering::Relaxed);

                // Calculate P99
                let mut sorted = times.clone();
                sorted.sort_unstable();
                let p99_index = (sorted.len() as f64 * 0.99) as usize;
                let p99 = sorted.get(p99_index).copied().unwrap_or(0);
                self.p99_response_time_us.store(p99, Ordering::Relaxed);
            }
        }
    }

    pub fn increment_in_flight(&self) {
        self.operations_in_flight.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_in_flight(&self) {
        self.operations_in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn generate_report(&self) -> RealTimeStatsReport {
        RealTimeStatsReport {
            operations_in_flight: self.operations_in_flight.load(Ordering::Relaxed),
            avg_response_time_us: self.avg_response_time_us.load(Ordering::Relaxed),
            p99_response_time_us: self.p99_response_time_us.load(Ordering::Relaxed),
        }
    }
}

/// Real-time statistics report
#[derive(Debug, Clone)]
pub struct RealTimeStatsReport {
    pub operations_in_flight: u64,
    pub avg_response_time_us: u64,
    pub p99_response_time_us: u64,
}
