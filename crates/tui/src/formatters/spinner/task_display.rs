//! Task display state and formatting logic

use super::constants::{PROGRESS_EMPTY, PROGRESS_FULL, SPINNER_FRAMES};
use crate::events::TaskState;
use crossterm::style::Color;
use std::time::Instant;

/// Task display state
#[derive(Clone, Debug)]
pub struct TaskDisplay {
    pub name: String,
    pub state: TaskState,
    pub message: Option<String>,
    pub progress: Option<f32>,
    pub depth: usize,
    pub dependencies: Vec<String>,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub line_number: Option<u16>,
    pub spinner_frame: usize,
    pub is_skipped: bool,
    pub skip_reason: Option<String>,
}

impl TaskDisplay {
    pub fn new(name: String, depth: usize, dependencies: Vec<String>) -> Self {
        Self {
            name,
            state: TaskState::Queued,
            message: None,
            progress: None,
            depth,
            dependencies,
            start_time: None,
            end_time: None,
            line_number: None,
            spinner_frame: 0,
            is_skipped: false,
            skip_reason: None,
        }
    }

    pub fn duration_str(&self) -> String {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => {
                let duration = end.duration_since(start);
                format!("{:.1}s", duration.as_secs_f32())
            }
            (Some(start), None) => {
                let duration = Instant::now().duration_since(start);
                format!("{:.1}s", duration.as_secs_f32())
            }
            _ => "0.0s".to_string(),
        }
    }

    pub fn status_icon(&self) -> &'static str {
        if self.is_skipped {
            "✔"
        } else {
            match self.state {
                TaskState::Queued => "◌",
                TaskState::Running => SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()],
                TaskState::Completed => "✔",
                TaskState::Failed => "✖",
                TaskState::Cancelled => "⊘",
            }
        }
    }

    pub fn status_color(&self) -> Color {
        if self.is_skipped {
            Color::Yellow
        } else {
            match self.state {
                TaskState::Queued => Color::DarkGrey,
                TaskState::Running => Color::Blue,
                TaskState::Completed => Color::Green,
                TaskState::Failed => Color::Red,
                TaskState::Cancelled => Color::DarkRed,
            }
        }
    }

    pub fn format_progress_bar(&self, width: usize) -> String {
        if let Some(progress) = self.progress {
            let filled = ((progress / 100.0) * width as f32) as usize;
            let empty = width.saturating_sub(filled);
            format!(
                "[{}{}]",
                PROGRESS_FULL.repeat(filled),
                PROGRESS_EMPTY.repeat(empty)
            )
        } else if self.state == TaskState::Running {
            // Show indeterminate progress spinner
            let pos = self.spinner_frame % (width * 2);
            let mut bar = vec![PROGRESS_EMPTY; width];

            // Create a wave effect
            for i in 0..3 {
                let idx = (pos + i) % width;
                if idx < width {
                    bar[idx] = PROGRESS_FULL;
                }
            }

            format!("[{}]", bar.join(""))
        } else {
            String::new()
        }
    }
}
