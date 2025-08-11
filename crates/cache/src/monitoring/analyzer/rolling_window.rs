//! Rolling window statistics for time-based hit rate analysis

use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct RollingWindow {
    hits: AtomicU64,
    misses: AtomicU64,
    window_start: RwLock<Instant>,
    window_duration: Duration,
}

impl RollingWindow {
    pub fn new(duration: Duration) -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            window_start: RwLock::new(Instant::now()),
            window_duration: duration,
        }
    }

    pub fn record_hit(&self) {
        self.roll_window();
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.roll_window();
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn hit_rate(&self) -> f64 {
        self.roll_window();
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    fn roll_window(&self) {
        let now = Instant::now();
        let mut window_start = self.window_start.write();

        if now.duration_since(*window_start) > self.window_duration {
            // Reset the window
            self.hits.store(0, Ordering::Relaxed);
            self.misses.store(0, Ordering::Relaxed);
            *window_start = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_window() {
        let window = RollingWindow::new(Duration::from_secs(1));

        // Record some hits and misses
        for _ in 0..7 {
            window.record_hit();
        }
        for _ in 0..3 {
            window.record_miss();
        }

        // Check hit rate
        assert!((window.hit_rate() - 0.7).abs() < 0.01);

        // Wait for window to expire
        std::thread::sleep(Duration::from_millis(1100));

        // After expiry, should reset
        assert_eq!(window.hit_rate(), 0.0);
    }
}
