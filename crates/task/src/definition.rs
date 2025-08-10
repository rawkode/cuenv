//! Task definition types for Phase 3 architecture
//!
//! This module defines the validated and immutable task definitions that result
//! from the task building process. These types represent "built" tasks ready for
//! execution, with all environment variables expanded, dependencies resolved,
//! and configurations validated.

use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Default task timeout in seconds (1 hour)
pub const DEFAULT_TASK_TIMEOUT_SECS: u64 = 3600;

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

/// Runtime task instance with execution state and metadata
#[derive(Debug, Clone)]
pub struct TaskInstance {
    /// The task definition
    pub definition: TaskDefinition,
    /// Current execution state
    pub state: TaskInstanceState,
    /// Execution metadata
    pub metadata: TaskInstanceMetadata,
}

/// Task instance execution state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskInstanceState {
    /// Task is waiting to be executed
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed { exit_code: i32 },
    /// Task failed with error
    Failed { exit_code: Option<i32>, error: String },
    /// Task was skipped (e.g., from cache)
    Skipped { reason: String },
}

/// Task instance metadata
#[derive(Debug, Clone)]
pub struct TaskInstanceMetadata {
    /// When the task was created
    pub created_at: std::time::SystemTime,
    /// When the task started executing (if it has)
    pub started_at: Option<std::time::SystemTime>,
    /// When the task finished executing (if it has)
    pub finished_at: Option<std::time::SystemTime>,
    /// Task execution duration
    pub duration: Option<Duration>,
    /// Additional metadata
    pub extra: HashMap<String, String>,
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
    /// Read-write paths (absolute)
    pub read_write_paths: Vec<PathBuf>,
    /// Denied paths (absolute)
    pub deny_paths: Vec<PathBuf>,
    /// Allowed network hosts
    pub allowed_hosts: Vec<String>,
}

/// Resolved cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCache {
    /// Whether caching is enabled
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
            timeout: Duration::from_secs(DEFAULT_TASK_TIMEOUT_SECS), // 1 hour default
        }
    }

    /// Get the command or script content for execution
    pub fn get_execution_content(&self) -> &str {
        match &self.execution_mode {
            TaskExecutionMode::Command { command } => command,
            TaskExecutionMode::Script { content } => content,
        }
    }

    /// Check if the task is cacheable
    pub fn is_cacheable(&self) -> bool {
        self.cache.enabled
    }

    /// Get security restrictions
    pub fn has_security_restrictions(&self) -> bool {
        self.security
            .as_ref()
            .map(|s| s.restrict_disk || s.restrict_network || !s.deny_paths.is_empty())
            .unwrap_or(false)
    }

    /// Get timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get dependency names
    pub fn dependency_names(&self) -> Vec<String> {
        self.dependencies.iter().map(|d| d.name.clone()).collect()
    }

    /// Get qualified dependency names (for cross-package dependencies)
    pub fn qualified_dependency_names(&self) -> Vec<String> {
        self.dependencies
            .iter()
            .map(|d| d.qualified_name.clone())
            .collect()
    }
}

impl TaskInstance {
    /// Create a new task instance
    pub fn new(definition: TaskDefinition) -> Self {
        Self {
            definition,
            state: TaskInstanceState::Pending,
            metadata: TaskInstanceMetadata {
                created_at: std::time::SystemTime::now(),
                started_at: None,
                finished_at: None,
                duration: None,
                extra: HashMap::new(),
            },
        }
    }

    /// Start the task execution
    pub fn start(&mut self) {
        self.state = TaskInstanceState::Running;
        self.metadata.started_at = Some(std::time::SystemTime::now());
    }

    /// Mark the task as completed
    pub fn complete(&mut self, exit_code: i32) {
        let now = std::time::SystemTime::now();
        self.metadata.finished_at = Some(now);
        
        if let Some(started_at) = self.metadata.started_at {
            self.metadata.duration = now.duration_since(started_at).ok();
        }
        
        self.state = TaskInstanceState::Completed { exit_code };
    }

    /// Mark the task as failed
    pub fn fail(&mut self, exit_code: Option<i32>, error: String) {
        let now = std::time::SystemTime::now();
        self.metadata.finished_at = Some(now);
        
        if let Some(started_at) = self.metadata.started_at {
            self.metadata.duration = now.duration_since(started_at).ok();
        }
        
        self.state = TaskInstanceState::Failed { exit_code, error };
    }

    /// Mark the task as skipped
    pub fn skip(&mut self, reason: String) {
        self.state = TaskInstanceState::Skipped { reason };
        self.metadata.finished_at = Some(std::time::SystemTime::now());
    }

    /// Check if the task is finished (completed, failed, or skipped)
    pub fn is_finished(&self) -> bool {
        matches!(
            self.state,
            TaskInstanceState::Completed { .. }
                | TaskInstanceState::Failed { .. }
                | TaskInstanceState::Skipped { .. }
        )
    }

    /// Get the exit code if the task is finished
    pub fn exit_code(&self) -> Option<i32> {
        match &self.state {
            TaskInstanceState::Completed { exit_code } => Some(*exit_code),
            TaskInstanceState::Failed { exit_code, .. } => *exit_code,
            _ => None,
        }
    }

    /// Check if the task succeeded
    pub fn succeeded(&self) -> bool {
        matches!(
            self.state,
            TaskInstanceState::Completed { exit_code: 0 } | TaskInstanceState::Skipped { .. }
        )
    }
}

impl ResolvedDependency {
    /// Create a new resolved dependency
    pub fn new(name: String) -> Self {
        Self {
            qualified_name: name.clone(),
            name,
            package: None,
        }
    }

    /// Create a new cross-package dependency
    pub fn with_package(name: String, package: String) -> Self {
        let qualified_name = format!("{}:{}", package, name);
        Self {
            name,
            package: Some(package),
            qualified_name,
        }
    }

    /// Check if this is a cross-package dependency
    pub fn is_cross_package(&self) -> bool {
        self.package.is_some()
    }
}

impl Default for TaskCache {
    fn default() -> Self {
        Self {
            enabled: true,
            key: None,
            env_filter: None,
        }
    }
}

impl From<&cuenv_config::TaskCacheConfig> for TaskCache {
    fn from(config: &cuenv_config::TaskCacheConfig) -> Self {
        Self {
            enabled: config.enabled(),
            key: None, // Custom keys not supported yet
            env_filter: config.env_filter().map(|_| CacheEnvFilter {
                include: Vec::new(),
                exclude: Vec::new(),
                smart_defaults: true,
            }),
        }
    }
}

impl TryFrom<TaskConfig> for TaskDefinition {
    type Error = Error;

    fn try_from(config: TaskConfig) -> Result<Self> {
        // Determine execution mode
        let execution_mode = match (&config.command, &config.script) {
            (Some(command), None) => TaskExecutionMode::Command {
                command: command.clone(),
            },
            (None, Some(script)) => TaskExecutionMode::Script {
                content: script.clone(),
            },
            (Some(_), Some(_)) => {
                return Err(Error::configuration(
                    "Task cannot have both 'command' and 'script' defined".to_string(),
                ));
            }
            (None, None) => {
                return Err(Error::configuration(
                    "Task must have either 'command' or 'script' defined".to_string(),
                ));
            }
        };

        // Convert dependencies
        let dependencies = config
            .dependencies
            .unwrap_or_default()
            .into_iter()
            .map(ResolvedDependency::new)
            .collect();

        // Convert security config
        let security = config.security.map(|s| TaskSecurity {
            restrict_disk: s.restrict_disk.unwrap_or(false),
            restrict_network: s.restrict_network.unwrap_or(false),
            read_only_paths: s
                .read_only_paths
                .unwrap_or_default()
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            read_write_paths: s
                .read_write_paths
                .unwrap_or_default()
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            deny_paths: s
                .deny_paths
                .unwrap_or_default()
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            allowed_hosts: s.allowed_hosts.unwrap_or_default(),
        });

        // Convert cache config
        let cache = config
            .cache
            .as_ref()
            .map(TaskCache::from)
            .unwrap_or_default();

        Ok(TaskDefinition {
            name: "".to_string(), // Will be set by TaskBuilder
            description: config.description,
            execution_mode,
            dependencies,
            working_directory: PathBuf::from("."), // Will be resolved by TaskBuilder
            shell: config.shell.unwrap_or_else(|| "sh".to_string()),
            inputs: config.inputs.unwrap_or_default(),
            outputs: config.outputs.unwrap_or_default(),
            security,
            cache,
            timeout: Duration::from_secs(config.timeout.unwrap_or(3600) as u64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_definition_creation() {
        let task = TaskDefinition::new(
            "test-task".to_string(),
            TaskExecutionMode::Command {
                command: "echo hello".to_string(),
            },
            PathBuf::from("/tmp"),
        );

        assert_eq!(task.name, "test-task");
        assert_eq!(task.get_execution_content(), "echo hello");
        assert!(task.is_cacheable());
        assert!(!task.has_security_restrictions());
    }

    #[test]
    fn test_task_instance_lifecycle() {
        let definition = TaskDefinition::new(
            "test-task".to_string(),
            TaskExecutionMode::Command {
                command: "echo hello".to_string(),
            },
            PathBuf::from("/tmp"),
        );

        let mut instance = TaskInstance::new(definition);
        assert!(matches!(instance.state, TaskInstanceState::Pending));
        assert!(!instance.is_finished());

        instance.start();
        assert!(matches!(instance.state, TaskInstanceState::Running));
        assert!(!instance.is_finished());

        instance.complete(0);
        assert!(matches!(
            instance.state,
            TaskInstanceState::Completed { exit_code: 0 }
        ));
        assert!(instance.is_finished());
        assert!(instance.succeeded());
        assert_eq!(instance.exit_code(), Some(0));
    }

    #[test]
    fn test_resolved_dependency() {
        let local_dep = ResolvedDependency::new("build".to_string());
        assert_eq!(local_dep.name, "build");
        assert_eq!(local_dep.qualified_name, "build");
        assert!(!local_dep.is_cross_package());

        let cross_dep = ResolvedDependency::with_package("test".to_string(), "pkg1".to_string());
        assert_eq!(cross_dep.name, "test");
        assert_eq!(cross_dep.qualified_name, "pkg1:test");
        assert!(cross_dep.is_cross_package());
    }
}