//! Spinner formatter module for task execution display
//!
//! This module provides a Docker Compose-style formatter with hierarchy display
//! for visualizing task execution progress with spinners and progress bars.

mod constants;
mod formatter;
mod task_display;

// Re-export the main formatter
pub use formatter::SpinnerFormatter;

// Include tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::TaskState;
    use crossterm::style::Color;
    use std::time::Instant;
    use task_display::TaskDisplay;

    #[test]
    fn test_task_display_new() {
        let dependencies = vec!["dep1".to_string(), "dep2".to_string()];
        let task = TaskDisplay::new("test_task".to_string(), 2, dependencies.clone());

        assert_eq!(task.name, "test_task");
        assert_eq!(task.depth, 2);
        assert_eq!(task.dependencies, dependencies);
        assert_eq!(task.state, TaskState::Queued);
        assert_eq!(task.spinner_frame, 0);
        assert!(!task.is_skipped);
    }

    #[test]
    fn test_task_display_duration_str() {
        let mut task = TaskDisplay::new("test".to_string(), 0, vec![]);

        // Test with no times set
        assert_eq!(task.duration_str(), "0.0s");

        // Test with start time only
        task.start_time = Some(Instant::now());
        let duration_str = task.duration_str();
        assert!(duration_str.ends_with('s'));

        // Test with both start and end times
        task.end_time = Some(task.start_time.unwrap() + std::time::Duration::from_secs(2));
        let duration_str = task.duration_str();
        assert!(duration_str.starts_with("2."));
        assert!(duration_str.ends_with('s'));
    }

    #[test]
    fn test_task_display_status_icon() {
        let mut task = TaskDisplay::new("test".to_string(), 0, vec![]);

        assert_eq!(task.status_icon(), "◌"); // Queued

        task.state = TaskState::Running;
        assert!(["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"].contains(&task.status_icon()));

        task.state = TaskState::Completed;
        assert_eq!(task.status_icon(), "✔");

        task.state = TaskState::Failed;
        assert_eq!(task.status_icon(), "✖");

        task.state = TaskState::Cancelled;
        assert_eq!(task.status_icon(), "⊘");

        // Test skipped override
        task.is_skipped = true;
        assert_eq!(task.status_icon(), "✔");
    }

    #[test]
    fn test_task_display_status_color() {
        let mut task = TaskDisplay::new("test".to_string(), 0, vec![]);

        assert_eq!(task.status_color(), Color::DarkGrey); // Queued

        task.state = TaskState::Running;
        assert_eq!(task.status_color(), Color::Blue);

        task.state = TaskState::Completed;
        assert_eq!(task.status_color(), Color::Green);

        task.state = TaskState::Failed;
        assert_eq!(task.status_color(), Color::Red);

        task.state = TaskState::Cancelled;
        assert_eq!(task.status_color(), Color::DarkRed);

        // Test skipped override
        task.is_skipped = true;
        assert_eq!(task.status_color(), Color::Yellow);
    }

    #[test]
    fn test_task_display_format_progress_bar() {
        let mut task = TaskDisplay::new("test".to_string(), 0, vec![]);

        // Test with no progress (default state)
        assert_eq!(task.format_progress_bar(10), "");

        // Test with specific progress
        task.progress = Some(50.0);
        let bar = task.format_progress_bar(10);
        assert!(bar.starts_with('['));
        assert!(bar.ends_with(']'));
        assert!(bar.contains('█'));
        assert!(bar.contains('░'));

        // Test with running state (indeterminate progress)
        task.progress = None;
        task.state = TaskState::Running;
        let bar = task.format_progress_bar(10);
        assert!(bar.starts_with('['));
        assert!(bar.ends_with(']'));
        assert!(bar.len() > 2); // More than just brackets
    }

    #[test]
    fn test_task_display_format_progress_bar_animation() {
        let mut task = TaskDisplay::new("test".to_string(), 0, vec![]);
        task.state = TaskState::Running;

        let bar1 = task.format_progress_bar(10);
        task.spinner_frame += 5;
        let bar2 = task.format_progress_bar(10);

        // Should be different due to animation
        assert_ne!(bar1, bar2);
    }

    #[test]
    fn test_spinner_frames_constant() {
        assert_eq!(constants::SPINNER_FRAMES.len(), 10);
        assert_eq!(constants::SPINNER_FRAMES[0], "⠋");
        assert_eq!(constants::SPINNER_FRAMES[9], "⠏");
    }
}
