//! Task execution events

use serde::{Deserialize, Serialize};

/// Task execution events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskEvent {
    /// A task has started execution
    TaskStarted { task_name: String, task_id: String },
    /// A task has completed successfully
    TaskCompleted {
        task_name: String,
        task_id: String,
        duration_ms: u64,
    },
    /// A task has failed
    TaskFailed {
        task_name: String,
        task_id: String,
        error: String,
    },
    /// Task progress update
    TaskProgress {
        task_name: String,
        task_id: String,
        message: String,
    },
    /// Task output (stdout)
    TaskOutput {
        task_name: String,
        task_id: String,
        output: String,
    },
    /// Task error output (stderr)
    TaskError {
        task_name: String,
        task_id: String,
        error: String,
    },
    /// Task skipped due to cache or conditions
    TaskSkipped {
        task_name: String,
        task_id: String,
        reason: String,
    },
}
