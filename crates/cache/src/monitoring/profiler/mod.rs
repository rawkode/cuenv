//! Performance profiler for flamegraph generation

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Performance profiler for flamegraph generation
pub struct PerformanceProfiler {
    /// Profiling data by operation
    profiles: RwLock<HashMap<String, ProfileData>>,
    /// Whether profiling is enabled
    enabled: AtomicU64,
    /// Sampling rate (1 in N operations)
    sampling_rate: u64,
}

struct ProfileData {
    samples: Vec<ProfileSample>,
    total_time: Duration,
    operation_count: u64,
}

struct ProfileSample {
    operation: String,
    duration: Duration,
    stack_trace: Vec<String>,
    _timestamp: Instant,
}

impl PerformanceProfiler {
    pub fn new() -> Self {
        Self {
            profiles: RwLock::new(HashMap::new()),
            enabled: AtomicU64::new(0),
            sampling_rate: 100, // Sample 1 in 100 operations
        }
    }

    pub fn should_profile(&self) -> bool {
        if self.enabled.load(Ordering::Relaxed) == 0 {
            return false;
        }

        // Simple sampling
        fastrand::u64(1..=self.sampling_rate) == 1
    }

    pub fn record_operation(&self, operation: &str, duration: Duration, hit: bool) {
        let sample = ProfileSample {
            operation: format!("{operation}_{}", if hit { "hit" } else { "miss" }),
            duration,
            stack_trace: Self::capture_stack_trace(),
            _timestamp: Instant::now(),
        };

        let mut profiles = self.profiles.write();
        let profile = profiles
            .entry(operation.to_string())
            .or_insert_with(|| ProfileData {
                samples: Vec::new(),
                total_time: Duration::ZERO,
                operation_count: 0,
            });

        profile.samples.push(sample);
        profile.total_time += duration;
        profile.operation_count += 1;

        // Keep only recent samples (last 10000)
        if profile.samples.len() > 10000 {
            profile.samples.drain(0..5000);
        }
    }

    pub fn enable(&self) {
        self.enabled.store(1, Ordering::Relaxed);
    }

    pub fn disable(&self) {
        self.enabled.store(0, Ordering::Relaxed);
    }

    pub fn generate_flamegraph(&self) -> String {
        let profiles = self.profiles.read();
        let mut output = String::new();

        for (_operation, profile) in profiles.iter() {
            for sample in &profile.samples {
                // Format: stack;frames;here count
                let stack = sample.stack_trace.join(";");
                let count = sample.duration.as_micros();
                output.push_str(&format!("{};{} {}\n", stack, sample.operation, count));
            }
        }

        output
    }

    fn capture_stack_trace() -> Vec<String> {
        // In a real implementation, this would capture the actual stack trace
        // For now, return a placeholder
        vec![
            "cuenv::cache::unified::get".to_string(),
            "cuenv::cache::streaming::read".to_string(),
            "tokio::runtime::Runtime::block_on".to_string(),
        ]
    }
}
