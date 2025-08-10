use std::time::{Duration, Instant};

/// Represents the current state of a task
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    /// Task is waiting to be executed (dependencies not met)
    Waiting,
    /// Task is currently running
    Running { started_at: Instant },
    /// Task completed successfully
    Completed {
        started_at: Instant,
        completed_at: Instant,
    },
    /// Task failed during execution
    Failed {
        started_at: Instant,
        failed_at: Instant,
        error: String,
    },
}

impl TaskState {
    /// Get the duration for completed or failed tasks
    pub fn duration(&self) -> Option<Duration> {
        match self {
            TaskState::Completed {
                started_at,
                completed_at,
            } => Some(*completed_at - *started_at),
            TaskState::Failed {
                started_at,
                failed_at,
                ..
            } => Some(*failed_at - *started_at),
            TaskState::Running { started_at } => Some(Instant::now() - *started_at),
            TaskState::Waiting => None,
        }
    }

    /// Check if the task is in a terminal state (completed or failed)
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Completed { .. } | TaskState::Failed { .. })
    }

    /// Check if the task is currently running
    pub fn is_running(&self) -> bool {
        matches!(self, TaskState::Running { .. })
    }

    /// Get a display symbol for the task state
    pub fn symbol(&self) -> &'static str {
        match self {
            TaskState::Waiting => "◯",
            TaskState::Running { .. } => "⟳",
            TaskState::Completed { .. } => "✓",
            TaskState::Failed { .. } => "✗",
        }
    }

    /// Get a display symbol for ASCII fallback
    pub fn symbol_ascii(&self) -> &'static str {
        match self {
            TaskState::Waiting => "o",
            TaskState::Running { .. } => "*",
            TaskState::Completed { .. } => "+",
            TaskState::Failed { .. } => "x",
        }
    }
}

/// Information about a task span for tree rendering
#[derive(Debug, Clone)]
pub struct TaskSpan {
    /// The task name
    pub name: String,
    /// Current state of the task
    pub state: TaskState,
    /// Progress percentage (0-100), if available
    pub progress: Option<u8>,
    /// Parent span ID, if this is a subtask
    pub parent_id: Option<u64>,
    /// Child span IDs
    pub children: Vec<u64>,
    /// Working directory or target information
    pub target: Option<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl TaskSpan {
    /// Create a new task span in waiting state
    pub fn new(name: String, parent_id: Option<u64>) -> Self {
        Self {
            name,
            state: TaskState::Waiting,
            progress: None,
            parent_id,
            children: Vec::new(),
            target: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Start the task (transition to running state)
    pub fn start(&mut self) {
        self.state = TaskState::Running {
            started_at: Instant::now(),
        };
    }

    /// Complete the task successfully
    pub fn complete(&mut self) {
        if let TaskState::Running { started_at } = self.state {
            self.state = TaskState::Completed {
                started_at,
                completed_at: Instant::now(),
            };
        }
    }

    /// Mark the task as failed
    pub fn fail(&mut self, error: String) {
        if let TaskState::Running { started_at } = self.state {
            self.state = TaskState::Failed {
                started_at,
                failed_at: Instant::now(),
                error,
            };
        }
    }

    /// Update task progress
    pub fn set_progress(&mut self, progress: u8) {
        self.progress = Some(progress.min(100));
    }

    /// Add a child span
    pub fn add_child(&mut self, child_id: u64) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    /// Get a formatted duration string
    pub fn duration_string(&self) -> String {
        match self.state.duration() {
            Some(duration) => {
                let secs = duration.as_secs_f32();
                if secs < 1.0 {
                    format!("{:.0}ms", duration.as_millis())
                } else {
                    format!("{secs:.1}s")
                }
            }
            None => String::new(),
        }
    }

    /// Get a progress bar string
    pub fn progress_bar(&self, width: usize) -> String {
        if let Some(progress) = self.progress {
            let filled = (width * progress as usize) / 100;
            let empty = width - filled;
            format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
        } else {
            String::new()
        }
    }

    /// Get a progress bar string with ASCII fallback
    pub fn progress_bar_ascii(&self, width: usize) -> String {
        if let Some(progress) = self.progress {
            let filled = (width * progress as usize) / 100;
            let empty = width - filled;
            format!("[{}{}]", "=".repeat(filled), "-".repeat(empty))
        } else {
            String::new()
        }
    }
}

/// Extract task name from a tracing span
pub fn extract_task_name(metadata: &tracing::Metadata<'_>) -> Option<String> {
    // Check if this is a task span
    if metadata.name() == "task" {
        // The task name should be in the span fields
        // This is a simplified version - in practice, we'd need to access the span's fields
        Some(metadata.target().to_string())
    } else {
        None
    }
}

/// Check if a span represents a task
pub fn is_task_span(metadata: &tracing::Metadata<'_>) -> bool {
    metadata.name() == "task"
}

/// Check if a span represents an execution level
pub fn is_level_span(metadata: &tracing::Metadata<'_>) -> bool {
    metadata.name() == "level"
}

/// Check if a span represents a pipeline
pub fn is_pipeline_span(metadata: &tracing::Metadata<'_>) -> bool {
    metadata.name() == "pipeline"
}
