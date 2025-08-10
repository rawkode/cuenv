use lazy_static::lazy_static;
use regex::Regex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::info;

lazy_static! {
    /// Regex for matching percentage patterns like "50%", "Progress: 75%"
    static ref PERCENTAGE_REGEX: Regex = Regex::new(r"(\d{1,3})%").unwrap();
    /// Regex for matching fraction patterns like "3/10", "completed 5 of 8"
    static ref FRACTION_REGEX: Regex = Regex::new(r"(\d+)(?:\s+of\s+|/)(\d+)").unwrap();
}

/// Progress tracking utilities for task execution monitoring
pub struct ProgressTracker {
    /// Last update timestamp to throttle updates
    last_update: AtomicU64,
    /// Minimum interval between progress updates (in milliseconds)
    throttle_ms: u64,
}

impl ProgressTracker {
    /// Create a new progress tracker with default throttling (100ms)
    pub fn new() -> Self {
        Self {
            last_update: AtomicU64::new(0),
            throttle_ms: 100,
        }
    }

    /// Create a new progress tracker with custom throttling interval
    pub fn with_throttle(throttle_ms: u64) -> Self {
        Self {
            last_update: AtomicU64::new(0),
            throttle_ms,
        }
    }

    /// Check if we should allow a progress update based on throttling
    pub fn should_update(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last = self.last_update.load(Ordering::Relaxed);

        if now - last >= self.throttle_ms {
            self.last_update.store(now, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Force an update regardless of throttling
    pub fn force_update(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_update.store(now, Ordering::Relaxed);
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse progress information from task output or messages
pub fn parse_progress(message: &str) -> Option<u8> {
    // Try to find percentage patterns like "50%", "Progress: 75%", etc.
    if let Some(captures) = PERCENTAGE_REGEX.captures(message) {
        if let Some(percent_str) = captures.get(1) {
            if let Ok(percent) = percent_str.as_str().parse::<u8>() {
                if percent <= 100 {
                    return Some(percent);
                }
            }
        }
    }

    // Try to find fraction patterns like "3/10", "completed 5 of 8", etc.
    if let Some(captures) = FRACTION_REGEX.captures(message) {
        if let (Some(current_str), Some(total_str)) = (captures.get(1), captures.get(2)) {
            if let (Ok(current), Ok(total)) = (
                current_str.as_str().parse::<u32>(),
                total_str.as_str().parse::<u32>(),
            ) {
                if total > 0 && current <= total {
                    let percent = ((current as f32 / total as f32) * 100.0) as u8;
                    return Some(percent);
                }
            }
        }
    }

    None
}

/// Aggregate progress from child tasks
///
/// Calculates the overall progress based on completed and running child tasks.
/// Tasks are weighted equally.
pub fn aggregate_progress(child_progresses: &[(String, Option<u8>, bool)]) -> Option<u8> {
    if child_progresses.is_empty() {
        return None;
    }

    let mut total_progress = 0.0;
    let task_count = child_progresses.len() as f32;

    for (_name, progress, completed) in child_progresses {
        if *completed {
            total_progress += 100.0;
        } else if let Some(p) = progress {
            total_progress += *p as f32;
        }
        // If a task has no progress and isn't completed, it contributes 0
    }

    Some((total_progress / task_count) as u8)
}

/// Create a visual progress indicator
pub fn create_progress_indicator(progress: Option<u8>, width: usize, use_unicode: bool) -> String {
    match progress {
        Some(percent) => {
            let filled = (width * percent as usize) / 100;
            let empty = width - filled;

            if use_unicode {
                format!("[{}{}] {}%", "█".repeat(filled), "░".repeat(empty), percent)
            } else {
                format!("[{}{}] {}%", "=".repeat(filled), "-".repeat(empty), percent)
            }
        }
        None => {
            if use_unicode {
                format!("[{}]", "░".repeat(width))
            } else {
                format!("[{}]", "-".repeat(width))
            }
        }
    }
}

/// Format duration for display
pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let millis = duration.subsec_millis();

    if total_secs == 0 {
        format!("{millis}ms")
    } else if total_secs < 60 {
        format!("{}.{}s", total_secs, millis / 100)
    } else {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{mins}m{secs}s")
    }
}

/// Progress reporting mechanism for tasks
pub struct TaskProgressReporter {
    task_name: String,
    tracker: ProgressTracker,
}

impl TaskProgressReporter {
    /// Create a new progress reporter for a task
    pub fn new(task_name: String) -> Self {
        Self {
            task_name,
            tracker: ProgressTracker::new(),
        }
    }

    /// Report progress if throttling allows
    pub fn report_progress(&self, progress: u8, message: &str) {
        if self.tracker.should_update() {
            info!(
                task_name = %self.task_name,
                progress_percent = %progress,
                message = %message,
                "task_progress"
            );
        }
    }

    /// Force progress report regardless of throttling
    pub fn force_report_progress(&self, progress: u8, message: &str) {
        self.tracker.force_update();
        info!(
            task_name = %self.task_name,
            progress_percent = %progress,
            message = %message,
            "task_progress"
        );
    }

    /// Report a message without progress
    pub fn report_message(&self, message: &str) {
        if self.tracker.should_update() {
            info!(
                task_name = %self.task_name,
                message = %message,
                "task_progress"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_progress_percentage() {
        assert_eq!(parse_progress("50%"), Some(50));
        assert_eq!(parse_progress("Progress: 75%"), Some(75));
        assert_eq!(parse_progress("Completed 100%"), Some(100));
        assert_eq!(parse_progress("0%"), Some(0));
        assert_eq!(parse_progress("150%"), None); // Invalid percentage
    }

    #[test]
    fn test_parse_progress_fraction() {
        assert_eq!(parse_progress("3/10"), Some(30));
        assert_eq!(parse_progress("completed 5 of 8"), Some(62));
        assert_eq!(parse_progress("1/2"), Some(50));
        assert_eq!(parse_progress("10/10"), Some(100));
    }

    #[test]
    fn test_aggregate_progress() {
        let progresses = vec![
            ("task1".to_string(), Some(50), false),
            ("task2".to_string(), Some(75), false),
            ("task3".to_string(), None, true),
        ];
        assert_eq!(aggregate_progress(&progresses), Some(75)); // (50 + 75 + 100) / 3
    }

    #[test]
    fn test_progress_indicator() {
        let indicator = create_progress_indicator(Some(50), 10, true);
        assert!(indicator.contains("█"));
        assert!(indicator.contains("░"));
        assert!(indicator.contains("50%"));

        let ascii_indicator = create_progress_indicator(Some(50), 10, false);
        assert!(ascii_indicator.contains("="));
        assert!(ascii_indicator.contains("-"));
        assert!(ascii_indicator.contains("50%"));
    }
}
