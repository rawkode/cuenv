//! Task-related types for execution pipeline management

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Default task timeout in seconds (1 hour)
pub const DEFAULT_TASK_TIMEOUT_SECS: u64 = 3600;

/// Task execution mode - either command or script
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskExecutionMode {
    /// Execute a command with arguments
    Command { command: String },
    /// Execute a script
    Script { content: String },
}

/// Dependency reference with package information (for future cross-package support)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedDependency {
    /// Dependency task name
    pub name: String,
    /// Package name (for cross-package dependencies)
    pub package: Option<String>,
    /// Full qualified name (package:task or just task)
    pub qualified_name: String,
}

impl ResolvedDependency {
    /// Create a new dependency without package information
    pub fn new(name: String) -> Self {
        Self {
            qualified_name: name.clone(),
            name,
            package: None,
        }
    }

    /// Create a new dependency with package information
    pub fn with_package(name: String, package: String) -> Self {
        let qualified_name = format!("{package}:{name}");
        Self {
            name,
            package: Some(package),
            qualified_name,
        }
    }
}

/// Validated security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSecurity {
    /// Restrict disk access
    pub restrict_disk: bool,
    /// Restrict network access
    pub restrict_network: bool,
    /// Read-only paths (absolute)
    pub read_only_paths: Vec<PathBuf>,
    /// Write-only paths (absolute)
    pub write_only_paths: Vec<PathBuf>,
    /// Allowed network hosts (for fine-grained control)
    pub allowed_hosts: Vec<String>,
}

/// Resolved cache configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskCache {
    /// Whether caching is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Custom cache key (if specified)
    pub key: Option<String>,
    /// Environment variable filtering for cache key computation
    pub env_filter: Option<CacheEnvFilter>,
}

/// Cache environment variable filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEnvFilter {
    /// Patterns to include in cache key
    pub include: Vec<String>,
    /// Patterns to exclude from cache key
    pub exclude: Vec<String>,
    /// Use smart defaults for common tools
    pub smart_defaults: bool,
}

/// Immutable, validated task definition ready for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    /// Task name
    pub name: String,
    /// Task description
    pub description: Option<String>,
    /// Execution mode (command or script)
    pub execution_mode: TaskExecutionMode,
    /// Resolved dependencies with package information
    pub dependencies: Vec<ResolvedDependency>,
    /// Working directory (absolute path)
    pub working_directory: PathBuf,
    /// Shell to use for execution
    pub shell: String,
    /// Input files/patterns
    pub inputs: Vec<String>,
    /// Output files/patterns  
    pub outputs: Vec<String>,
    /// Security configuration
    pub security: Option<TaskSecurity>,
    /// Cache configuration
    pub cache: TaskCache,
    /// Timeout for execution
    pub timeout: Duration,
}

impl TaskDefinition {
    /// Create a new task definition
    pub fn new(
        name: String,
        execution_mode: TaskExecutionMode,
        working_directory: PathBuf,
    ) -> Self {
        Self {
            name,
            description: None,
            execution_mode,
            dependencies: Vec::new(),
            working_directory,
            shell: "sh".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            security: None,
            cache: TaskCache::default(),
            timeout: Duration::from_secs(DEFAULT_TASK_TIMEOUT_SECS),
        }
    }

    /// Get the command or script content for execution
    pub fn get_execution_content(&self) -> &str {
        match &self.execution_mode {
            TaskExecutionMode::Command { command } => command,
            TaskExecutionMode::Script { content } => content,
        }
    }

    /// Check if this task is a command execution
    pub fn is_command(&self) -> bool {
        matches!(self.execution_mode, TaskExecutionMode::Command { .. })
    }

    /// Check if this task is a script execution
    pub fn is_script(&self) -> bool {
        matches!(self.execution_mode, TaskExecutionMode::Script { .. })
    }

    /// Get the names of all dependencies
    pub fn dependency_names(&self) -> Vec<String> {
        self.dependencies
            .iter()
            .map(|dep| dep.name.clone())
            .collect()
    }
}
