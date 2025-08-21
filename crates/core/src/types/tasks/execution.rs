//! Task execution-related types and functionality

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Task execution mode - either command or script
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskExecutionMode {
    /// Execute a command with arguments
    Command { command: String },
    /// Execute a script
    Script { content: String },
}

impl TaskExecutionMode {
    /// Get the execution content (command or script)
    pub fn get_content(&self) -> &str {
        match self {
            TaskExecutionMode::Command { command } => command,
            TaskExecutionMode::Script { content } => content,
        }
    }

    /// Check if this is a command execution
    pub fn is_command(&self) -> bool {
        matches!(self, TaskExecutionMode::Command { .. })
    }

    /// Check if this is a script execution
    pub fn is_script(&self) -> bool {
        matches!(self, TaskExecutionMode::Script { .. })
    }
}
