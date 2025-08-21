//! Type-safe state machines using phantom types

use crate::errors::{Error, Result};
use std::fmt;
use std::marker::PhantomData;

/// Phantom type marker for uninitialized state
#[derive(Debug)]
pub struct Uninitialized;

/// Phantom type marker for initialized state  
#[derive(Debug)]
pub struct Initialized;

/// Phantom type marker for running state
#[derive(Debug)]
pub struct Running;

/// Phantom type marker for completed state
#[derive(Debug)]
pub struct Completed;

/// Phantom type marker for failed state
#[derive(Debug)]
pub struct Failed;

/// A type-safe task execution builder using phantom types
#[derive(Debug)]
pub struct TaskExecution<State = Uninitialized> {
    name: Option<String>,
    command: Option<String>,
    args: Vec<String>,
    working_dir: Option<String>,
    timeout_seconds: Option<u32>,
    _state: PhantomData<State>,
}

impl TaskExecution<Uninitialized> {
    /// Create a new task execution builder
    pub fn new() -> Self {
        Self {
            name: None,
            command: None,
            args: Vec::new(),
            working_dir: None,
            timeout_seconds: None,
            _state: PhantomData,
        }
    }

    /// Set the task name and move to initialized state
    pub fn with_name(mut self, name: impl Into<String>) -> TaskExecution<Initialized> {
        self.name = Some(name.into());
        TaskExecution {
            name: self.name,
            command: self.command,
            args: self.args,
            working_dir: self.working_dir,
            timeout_seconds: self.timeout_seconds,
            _state: PhantomData,
        }
    }
}

impl TaskExecution<Initialized> {
    /// Set the command to execute
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Add arguments
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(|s| s.into()));
        self
    }

    /// Set working directory
    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, seconds: u32) -> Self {
        self.timeout_seconds = Some(seconds);
        self
    }

    /// Start execution (move to running state)
    pub fn start(self) -> Result<TaskExecution<Running>> {
        if self.command.is_none() {
            return Err(Error::Configuration {
                message: "Command must be set before starting execution".to_string(),
            });
        }

        Ok(TaskExecution {
            name: self.name,
            command: self.command,
            args: self.args,
            working_dir: self.working_dir,
            timeout_seconds: self.timeout_seconds,
            _state: PhantomData,
        })
    }
}

impl TaskExecution<Running> {
    /// Complete the task successfully
    pub fn complete(self) -> TaskExecution<Completed> {
        TaskExecution {
            name: self.name,
            command: self.command,
            args: self.args,
            working_dir: self.working_dir,
            timeout_seconds: self.timeout_seconds,
            _state: PhantomData,
        }
    }

    /// Fail the task
    pub fn fail(self) -> TaskExecution<Failed> {
        TaskExecution {
            name: self.name,
            command: self.command,
            args: self.args,
            working_dir: self.working_dir,
            timeout_seconds: self.timeout_seconds,
            _state: PhantomData,
        }
    }

    /// Get task information while running
    pub fn info(&self) -> TaskInfo {
        TaskInfo {
            name: self.name.as_deref().unwrap_or("unnamed"),
            command: self.command.as_deref().unwrap_or(""),
            args: &self.args,
            working_dir: self.working_dir.as_deref(),
            timeout_seconds: self.timeout_seconds,
        }
    }
}

impl TaskExecution<Completed> {
    /// Get the completed task name
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("unnamed")
    }
}

impl TaskExecution<Failed> {
    /// Get the failed task name
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("unnamed")
    }

    /// Retry the task (move back to initialized state)
    pub fn retry(self) -> TaskExecution<Initialized> {
        TaskExecution {
            name: self.name,
            command: self.command,
            args: self.args,
            working_dir: self.working_dir,
            timeout_seconds: self.timeout_seconds,
            _state: PhantomData,
        }
    }
}

impl Default for TaskExecution<Uninitialized> {
    fn default() -> Self {
        Self::new()
    }
}

/// Task information struct for runtime access
#[derive(Debug, Clone)]
pub struct TaskInfo<'a> {
    pub name: &'a str,
    pub command: &'a str,
    pub args: &'a [String],
    pub working_dir: Option<&'a str>,
    pub timeout_seconds: Option<u32>,
}

/// A type-safe configuration builder using phantom types
#[derive(Debug)]
pub struct ConfigBuilder<State = Uninitialized> {
    env_vars: Vec<(String, String)>,
    hooks: Vec<String>,
    tasks: Vec<String>,
    validated: bool,
    _state: PhantomData<State>,
}

impl ConfigBuilder<Uninitialized> {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            env_vars: Vec::new(),
            hooks: Vec::new(),
            tasks: Vec::new(),
            validated: false,
            _state: PhantomData,
        }
    }

    /// Add environment variables
    pub fn with_env_vars<I>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        self.env_vars.extend(vars);
        self
    }

    /// Add hooks
    pub fn with_hooks<I, S>(mut self, hooks: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.hooks.extend(hooks.into_iter().map(|s| s.into()));
        self
    }

    /// Add tasks
    pub fn with_tasks<I, S>(mut self, tasks: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tasks.extend(tasks.into_iter().map(|s| s.into()));
        self
    }

    /// Validate the configuration and move to initialized state
    pub fn validate(mut self) -> Result<ConfigBuilder<Initialized>> {
        // Perform validation logic here
        if self.env_vars.is_empty() && self.hooks.is_empty() && self.tasks.is_empty() {
            return Err(Error::Configuration {
                message: "Configuration must have at least one environment variable, hook, or task"
                    .to_string(),
            });
        }

        self.validated = true;
        Ok(ConfigBuilder {
            env_vars: self.env_vars,
            hooks: self.hooks,
            tasks: self.tasks,
            validated: self.validated,
            _state: PhantomData,
        })
    }
}

impl ConfigBuilder<Initialized> {
    /// Build the final configuration
    pub fn build(self) -> Configuration {
        Configuration {
            env_vars: self.env_vars,
            hooks: self.hooks,
            tasks: self.tasks,
        }
    }

    /// Get a preview of the configuration
    pub fn preview(&self) -> ConfigPreview {
        ConfigPreview {
            env_var_count: self.env_vars.len(),
            hook_count: self.hooks.len(),
            task_count: self.tasks.len(),
        }
    }
}

impl Default for ConfigBuilder<Uninitialized> {
    fn default() -> Self {
        Self::new()
    }
}

/// Final configuration structure
#[derive(Debug, Clone)]
pub struct Configuration {
    pub env_vars: Vec<(String, String)>,
    pub hooks: Vec<String>,
    pub tasks: Vec<String>,
}

/// Configuration preview for display
#[derive(Debug, Clone)]
pub struct ConfigPreview {
    pub env_var_count: usize,
    pub hook_count: usize,
    pub task_count: usize,
}

impl fmt::Display for ConfigPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Configuration Preview: {} env vars, {} hooks, {} tasks",
            self.env_var_count, self.hook_count, self.task_count
        )
    }
}

/// Type-safe state trait for generic operations
pub trait State: fmt::Debug {
    /// Get the state name
    fn state_name() -> &'static str;
}

impl State for Uninitialized {
    fn state_name() -> &'static str {
        "Uninitialized"
    }
}

impl State for Initialized {
    fn state_name() -> &'static str {
        "Initialized"
    }
}

impl State for Running {
    fn state_name() -> &'static str {
        "Running"
    }
}

impl State for Completed {
    fn state_name() -> &'static str {
        "Completed"
    }
}

impl State for Failed {
    fn state_name() -> &'static str {
        "Failed"
    }
}

/// Generic state machine operations
pub trait StateMachine<S: State> {
    /// Get the current state name
    fn current_state(&self) -> &'static str {
        S::state_name()
    }
}

impl<S: State> StateMachine<S> for TaskExecution<S> {}
impl<S: State> StateMachine<S> for ConfigBuilder<S> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_execution_state_machine() {
        // This should compile - valid state transitions
        let task = TaskExecution::new()
            .with_name("test_task")
            .with_command("echo")
            .with_args(["hello", "world"])
            .start()
            .unwrap()
            .complete();

        assert_eq!(task.name(), "test_task");
        assert_eq!(task.current_state(), "Completed");
    }

    #[test]
    fn test_task_execution_failure() {
        let task = TaskExecution::new()
            .with_name("failing_task")
            .with_command("false")
            .start()
            .unwrap()
            .fail()
            .retry();

        assert_eq!(task.current_state(), "Initialized");
    }

    #[test]
    fn test_config_builder_state_machine() {
        let config = ConfigBuilder::new()
            .with_env_vars([("TEST_VAR".to_string(), "test_value".to_string())])
            .validate()
            .unwrap()
            .build();

        assert_eq!(config.env_vars.len(), 1);
        assert_eq!(
            config.env_vars[0],
            ("TEST_VAR".to_string(), "test_value".to_string())
        );
    }

    #[test]
    fn test_config_validation_failure() {
        let result = ConfigBuilder::new().validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_task_without_command_fails() {
        let result = TaskExecution::new().with_name("incomplete_task").start();
        assert!(result.is_err());
    }
}
