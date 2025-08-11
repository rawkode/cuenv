//! Pipeline execution events

use serde::{Deserialize, Serialize};

/// Pipeline execution events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineEvent {
    /// Pipeline execution started
    PipelineStarted {
        total_tasks: usize,
        total_levels: usize,
    },
    /// A level of tasks started
    LevelStarted { level: usize, tasks_in_level: usize },
    /// A level of tasks completed
    LevelCompleted {
        level: usize,
        successful_tasks: usize,
        failed_tasks: usize,
    },
    /// Pipeline execution completed
    PipelineCompleted {
        total_duration_ms: u64,
        successful_tasks: usize,
        failed_tasks: usize,
    },
}
