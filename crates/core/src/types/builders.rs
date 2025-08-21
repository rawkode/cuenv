//! Functional builders using typestate pattern for compile-time safety

use crate::errors::{Error, Result, Validate};
use crate::types::newtypes::{EnvVarName, TaskName, TimeoutSecondsNewtype, ValidatedPath};
use std::collections::HashMap;
use std::marker::PhantomData;

/// State markers for the builder pattern
pub mod states {
    /// Initial state - no required fields set
    pub struct Initial;

    /// Name has been set
    pub struct WithName;

    /// Command has been set (requires name first)
    pub struct WithCommand;

    /// Builder is complete and ready to build
    pub struct Ready;
}

/// A functional task configuration builder using typestate pattern
#[derive(Debug)]
pub struct TaskBuilder<State = states::Initial> {
    name: Option<TaskName>,
    description: Option<String>,
    command: Option<String>,
    args: Vec<String>,
    working_dir: Option<ValidatedPath>,
    env_vars: HashMap<EnvVarName, String>,
    timeout: Option<TimeoutSecondsNewtype>,
    dependencies: Vec<TaskName>,
    inputs: Vec<ValidatedPath>,
    outputs: Vec<ValidatedPath>,
    _state: PhantomData<State>,
}

impl TaskBuilder<states::Initial> {
    /// Create a new task builder
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            command: None,
            args: Vec::new(),
            working_dir: None,
            env_vars: HashMap::new(),
            timeout: None,
            dependencies: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            _state: PhantomData,
        }
    }

    /// Set the task name and move to WithName state
    pub fn with_name<N>(self, name: N) -> Result<TaskBuilder<states::WithName>>
    where
        N: TryInto<TaskName>,
        N::Error: Into<Error>,
    {
        let name = name.try_into().map_err(Into::into)?;

        Ok(TaskBuilder {
            name: Some(name),
            description: self.description,
            command: self.command,
            args: self.args,
            working_dir: self.working_dir,
            env_vars: self.env_vars,
            timeout: self.timeout,
            dependencies: self.dependencies,
            inputs: self.inputs,
            outputs: self.outputs,
            _state: PhantomData,
        })
    }
}

impl TaskBuilder<states::WithName> {
    /// Set the command and move to WithCommand state
    pub fn with_command(self, command: impl Into<String>) -> TaskBuilder<states::WithCommand> {
        TaskBuilder {
            name: self.name,
            description: self.description,
            command: Some(command.into()),
            args: self.args,
            working_dir: self.working_dir,
            env_vars: self.env_vars,
            timeout: self.timeout,
            dependencies: self.dependencies,
            inputs: self.inputs,
            outputs: self.outputs,
            _state: PhantomData,
        }
    }

    /// Set optional description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add dependencies
    pub fn with_dependencies<I>(mut self, deps: I) -> Result<Self>
    where
        I: IntoIterator,
        I::Item: TryInto<TaskName>,
        <I::Item as TryInto<TaskName>>::Error: Into<Error>,
    {
        for dep in deps {
            let task_name = dep.try_into().map_err(Into::into)?;
            self.dependencies.push(task_name);
        }
        Ok(self)
    }
}

impl TaskBuilder<states::WithCommand> {
    /// Add command arguments
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Set working directory
    pub fn with_working_dir<D>(mut self, dir: D) -> Result<Self>
    where
        D: TryInto<ValidatedPath>,
        D::Error: Into<Error>,
    {
        self.working_dir = Some(dir.try_into().map_err(Into::into)?);
        Ok(self)
    }

    /// Add environment variables
    pub fn with_env_vars<I>(mut self, vars: I) -> Result<Self>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        for (key, value) in vars {
            let env_name = EnvVarName::new(key)?;
            self.env_vars.insert(env_name, value);
        }
        Ok(self)
    }

    /// Add a single environment variable
    pub fn with_env_var<K, V>(mut self, key: K, value: V) -> Result<Self>
    where
        K: TryInto<EnvVarName>,
        K::Error: Into<Error>,
        V: Into<String>,
    {
        let env_name = key.try_into().map_err(Into::into)?;
        self.env_vars.insert(env_name, value.into());
        Ok(self)
    }

    /// Set timeout
    pub fn with_timeout<T>(mut self, timeout: T) -> Result<Self>
    where
        T: TryInto<TimeoutSecondsNewtype>,
        T::Error: Into<Error>,
    {
        self.timeout = Some(timeout.try_into().map_err(Into::into)?);
        Ok(self)
    }

    /// Add input files
    pub fn with_inputs<I, P>(mut self, inputs: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: TryInto<ValidatedPath>,
        P::Error: Into<Error>,
    {
        for input in inputs {
            let path = input.try_into().map_err(Into::into)?;
            self.inputs.push(path);
        }
        Ok(self)
    }

    /// Add output files
    pub fn with_outputs<I, P>(mut self, outputs: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: TryInto<ValidatedPath>,
        P::Error: Into<Error>,
    {
        for output in outputs {
            let path = output.try_into().map_err(Into::into)?;
            self.outputs.push(path);
        }
        Ok(self)
    }

    /// Mark as ready for building
    pub fn ready(self) -> TaskBuilder<states::Ready> {
        TaskBuilder {
            name: self.name,
            description: self.description,
            command: self.command,
            args: self.args,
            working_dir: self.working_dir,
            env_vars: self.env_vars,
            timeout: self.timeout,
            dependencies: self.dependencies,
            inputs: self.inputs,
            outputs: self.outputs,
            _state: PhantomData,
        }
    }
}

impl TaskBuilder<states::Ready> {
    /// Build the final task configuration
    pub fn build(self) -> Result<TaskConfig> {
        let name = self.name.ok_or_else(|| Error::Configuration {
            message: "Task name is required".to_string(),
        })?;

        let command = self.command.ok_or_else(|| Error::Configuration {
            message: "Task command is required".to_string(),
        })?;

        Ok(TaskConfig {
            name,
            description: self.description,
            command,
            args: self.args,
            working_dir: self.working_dir,
            env_vars: self.env_vars,
            timeout: self.timeout,
            dependencies: self.dependencies,
            inputs: self.inputs,
            outputs: self.outputs,
        })
    }

    /// Build with validation
    pub fn build_validated(self) -> Result<TaskConfig> {
        let config = self.build()?;
        config.validate()?;
        Ok(config)
    }
}

impl Default for TaskBuilder<states::Initial> {
    fn default() -> Self {
        Self::new()
    }
}

/// Final task configuration
#[derive(Debug, Clone)]
pub struct TaskConfig {
    pub name: TaskName,
    pub description: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<ValidatedPath>,
    pub env_vars: HashMap<EnvVarName, String>,
    pub timeout: Option<TimeoutSecondsNewtype>,
    pub dependencies: Vec<TaskName>,
    pub inputs: Vec<ValidatedPath>,
    pub outputs: Vec<ValidatedPath>,
}

impl TaskConfig {
    /// Validate the task configuration
    pub fn validate(&self) -> Result<()> {
        // Validate command is not empty
        Validate::not_empty(&self.command, "command")?;

        // Validate dependencies don't include self
        if self.dependencies.iter().any(|dep| dep == &self.name) {
            return Err(Error::Configuration {
                message: format!("Task '{}' cannot depend on itself", self.name),
            });
        }

        // Validate working directory exists if specified
        if let Some(ref dir) = self.working_dir {
            if !dir.exists() {
                return Err(Error::Configuration {
                    message: format!("Working directory '{}' does not exist", dir),
                });
            }
        }

        Ok(())
    }
}

/// A functional environment configuration builder
#[derive(Debug)]
pub struct EnvBuilder<State = states::Initial> {
    variables: HashMap<EnvVarName, String>,
    hooks_on_enter: Vec<String>,
    hooks_on_exit: Vec<String>,
    tasks: HashMap<TaskName, TaskConfig>,
    _state: PhantomData<State>,
}

impl EnvBuilder<states::Initial> {
    /// Create a new environment builder
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            hooks_on_enter: Vec::new(),
            hooks_on_exit: Vec::new(),
            tasks: HashMap::new(),
            _state: PhantomData,
        }
    }

    /// Add environment variables and progress to the next state
    pub fn with_env_vars<I>(mut self, vars: I) -> Result<EnvBuilder<states::WithName>>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        for (key, value) in vars {
            let env_name = EnvVarName::new(key)?;
            self.variables.insert(env_name, value);
        }

        Ok(EnvBuilder {
            variables: self.variables,
            hooks_on_enter: self.hooks_on_enter,
            hooks_on_exit: self.hooks_on_exit,
            tasks: self.tasks,
            _state: PhantomData,
        })
    }
}

impl EnvBuilder<states::WithName> {
    /// Add hooks for environment entry
    pub fn with_enter_hooks<I, S>(mut self, hooks: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.hooks_on_enter
            .extend(hooks.into_iter().map(Into::into));
        self
    }

    /// Add hooks for environment exit
    pub fn with_exit_hooks<I, S>(mut self, hooks: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.hooks_on_exit.extend(hooks.into_iter().map(Into::into));
        self
    }

    /// Add tasks
    pub fn with_tasks<I>(mut self, tasks: I) -> Result<Self>
    where
        I: IntoIterator<Item = TaskConfig>,
    {
        for task in tasks {
            self.tasks.insert(task.name.clone(), task);
        }
        Ok(self)
    }

    /// Mark as ready
    pub fn ready(self) -> EnvBuilder<states::Ready> {
        EnvBuilder {
            variables: self.variables,
            hooks_on_enter: self.hooks_on_enter,
            hooks_on_exit: self.hooks_on_exit,
            tasks: self.tasks,
            _state: PhantomData,
        }
    }
}

impl EnvBuilder<states::Ready> {
    /// Build the final environment configuration
    pub fn build(self) -> EnvConfig {
        EnvConfig {
            variables: self.variables,
            hooks_on_enter: self.hooks_on_enter,
            hooks_on_exit: self.hooks_on_exit,
            tasks: self.tasks,
        }
    }
}

impl Default for EnvBuilder<states::Initial> {
    fn default() -> Self {
        Self::new()
    }
}

/// Final environment configuration
#[derive(Debug, Clone)]
pub struct EnvConfig {
    pub variables: HashMap<EnvVarName, String>,
    pub hooks_on_enter: Vec<String>,
    pub hooks_on_exit: Vec<String>,
    pub tasks: HashMap<TaskName, TaskConfig>,
}

impl EnvConfig {
    /// Get all task names in dependency order
    pub fn task_execution_order(&self) -> Result<Vec<TaskName>> {
        // Simplified topological sort - in a real implementation, you'd need proper cycle detection
        let mut ordered = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for task_name in self.tasks.keys() {
            if !visited.contains(task_name) {
                self.visit_task(task_name, &mut visited, &mut ordered)?;
            }
        }

        Ok(ordered)
    }

    fn visit_task(
        &self,
        task_name: &TaskName,
        visited: &mut std::collections::HashSet<TaskName>,
        ordered: &mut Vec<TaskName>,
    ) -> Result<()> {
        if visited.contains(task_name) {
            return Ok(());
        }

        let task = self
            .tasks
            .get(task_name)
            .ok_or_else(|| Error::Configuration {
                message: format!("Task '{}' not found", task_name),
            })?;

        // Visit dependencies first
        for dep in &task.dependencies {
            self.visit_task(dep, visited, ordered)?;
        }

        visited.insert(task_name.clone());
        ordered.push(task_name.clone());

        Ok(())
    }
}

/// Functional composition utilities for builders
pub trait BuilderExt<T> {
    /// Apply a transformation function if a condition is true
    fn when<F>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
        Self: Sized;

    /// Apply a fallible transformation function
    fn try_apply<F>(self, f: F) -> Result<Self>
    where
        F: FnOnce(Self) -> Result<Self>,
        Self: Sized;

    /// Chain multiple builder operations
    fn chain<F>(self, operations: Vec<F>) -> Result<Self>
    where
        F: FnOnce(Self) -> Result<Self>,
        Self: Sized;
}

impl<T, State> BuilderExt<T> for TaskBuilder<State> {
    fn when<F>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if condition {
            f(self)
        } else {
            self
        }
    }

    fn try_apply<F>(self, f: F) -> Result<Self>
    where
        F: FnOnce(Self) -> Result<Self>,
    {
        f(self)
    }

    fn chain<F>(self, operations: Vec<F>) -> Result<Self>
    where
        F: FnOnce(Self) -> Result<Self>,
    {
        operations
            .into_iter()
            .try_fold(self, |builder, op| op(builder))
    }
}

impl<T, State> BuilderExt<T> for EnvBuilder<State> {
    fn when<F>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if condition {
            f(self)
        } else {
            self
        }
    }

    fn try_apply<F>(self, f: F) -> Result<Self>
    where
        F: FnOnce(Self) -> Result<Self>,
    {
        f(self)
    }

    fn chain<F>(self, operations: Vec<F>) -> Result<Self>
    where
        F: FnOnce(Self) -> Result<Self>,
    {
        operations
            .into_iter()
            .try_fold(self, |builder, op| op(builder))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_builder_state_machine() -> Result<()> {
        let task = TaskBuilder::new()
            .with_name("test_task")?
            .with_command("echo")
            .with_args(["hello", "world"])
            .ready()
            .build()?;

        assert_eq!(task.name.as_str(), "test_task");
        assert_eq!(task.command, "echo");
        assert_eq!(task.args, vec!["hello", "world"]);

        Ok(())
    }

    #[test]
    fn test_env_builder() -> Result<()> {
        let env = EnvBuilder::new()
            .with_env_vars([("PATH".to_string(), "/usr/bin".to_string())])?
            .with_enter_hooks(["echo entering"])
            .ready()
            .build();

        assert_eq!(env.variables.len(), 1);
        assert_eq!(env.hooks_on_enter, vec!["echo entering"]);

        Ok(())
    }

    #[test]
    fn test_builder_extensions() -> Result<()> {
        let mut builder = TaskBuilder::new()
            .with_name("conditional_task")?
            .with_command("ls");

        if true {
            builder = builder.with_args(["-la"]);
        }

        let task = builder.ready().build()?;

        assert_eq!(task.args, vec!["-la"]);

        Ok(())
    }

    #[test]
    fn test_task_validation() -> Result<()> {
        let task_name = TaskName::new("self_dependent")?;
        let mut task = TaskBuilder::new()
            .with_name("self_dependent")?
            .with_command("echo")
            .ready()
            .build()?;

        // Manually add self-dependency to test validation
        task.dependencies.push(task_name);

        assert!(task.validate().is_err());

        Ok(())
    }
}
