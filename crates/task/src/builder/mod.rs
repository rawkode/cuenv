//! Task Builder for Phase 3 architecture
//!
//! This module provides the TaskBuilder that separates task building from execution.
//! It takes raw TaskConfig objects and produces validated, immutable TaskDefinition
//! objects ready for execution.

use cuenv_config::TaskConfig;
use cuenv_core::{Result, TaskDefinition};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

// Re-export the focused modules
pub mod conversion;
pub mod dependency;
pub mod env_expansion;
pub mod security;
pub mod validation;

// Re-export the main types and functions from modules
pub use dependency::{create_dependency_cache, DependencyValidationCache};

/// Task building context
#[derive(Debug)]
pub struct BuildContext {
    /// Task configurations by name
    pub task_configs: HashMap<String, TaskConfig>,
    /// Resolved task definitions by name
    pub task_definitions: HashMap<String, TaskDefinition>,
    /// Dependency graph for validation
    pub dependency_graph: HashMap<String, Vec<String>>,
}

/// Task builder that validates and builds task definitions from configurations
#[derive(Clone)]
pub struct TaskBuilder {
    /// Workspace root directory
    workspace_root: PathBuf,
    /// Global environment variables for expansion
    global_env: HashMap<String, String>,
    /// Cached dependency validation results
    dependency_cache: DependencyValidationCache,
}

impl TaskBuilder {
    /// Create a new task builder
    pub fn new(workspace_root: PathBuf) -> Self {
        let global_env = env::vars().collect();
        Self::new_with_env(workspace_root, global_env)
    }

    /// Create a new task builder with custom environment
    pub fn new_with_env(workspace_root: PathBuf, global_env: HashMap<String, String>) -> Self {
        Self {
            workspace_root,
            global_env,
            dependency_cache: create_dependency_cache(),
        }
    }

    /// Build task definitions from configurations
    pub fn build_tasks(
        &self,
        task_configs: HashMap<String, TaskConfig>,
    ) -> Result<HashMap<String, TaskDefinition>> {
        let mut context = BuildContext {
            task_configs: task_configs.clone(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        // Step 1: Validate task configurations
        validation::validate_task_configs(&task_configs)?;

        // Step 2: Build initial task definitions
        for (name, config) in &task_configs {
            let mut definition = conversion::config_to_definition(config.clone())?;
            definition.name = name.clone();

            // Validate the conversion was successful
            conversion::validate_conversion(&definition)?;

            context.task_definitions.insert(name.clone(), definition);
        }

        // Step 3: Resolve dependencies
        dependency::resolve_dependencies(&mut context)?;

        // Step 4: Validate dependency graph for cycles
        dependency::validate_dependencies(&context, &self.dependency_cache)?;

        // Step 5: Expand environment variables
        env_expansion::expand_environment_variables(&mut context, &self.global_env)?;

        // Step 6: Resolve working directories
        env_expansion::resolve_working_directories(
            &mut context,
            &self.workspace_root,
            &self.global_env,
        )?;

        // Step 7: Validate security configurations
        security::validate_security_configs(&mut context, &self.workspace_root)?;

        Ok(context.task_definitions)
    }

    /// Get workspace root directory
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Update global environment variables
    pub fn update_env(&mut self, env_vars: HashMap<String, String>) {
        self.global_env.extend(env_vars);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::{SecurityConfig, TaskCacheConfig};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config(command: &str) -> TaskConfig {
        TaskConfig {
            description: Some("Test task".to_string()),
            command: Some(command.to_string()),
            script: None,
            dependencies: None,
            working_dir: None,
            shell: Some("sh".to_string()),
            inputs: None,
            outputs: None,
            security: None,
            cache: Some(TaskCacheConfig::Simple(true)),
            cache_key: None,
            cache_env: None,
            timeout: Some(30),
        }
    }

    #[test]
    fn test_task_builder_basic() {
        let temp_dir = TempDir::new().unwrap();
        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut configs = HashMap::new();
        configs.insert("test".to_string(), create_test_config("echo hello"));

        let definitions = builder.build_tasks(configs).unwrap();

        assert_eq!(definitions.len(), 1);
        let definition = &definitions["test"];
        assert_eq!(definition.name, "test");
        assert_eq!(definition.get_execution_content(), "echo hello");
        assert_eq!(definition.shell, "sh");
    }

    #[test]
    fn test_dependency_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut configs = HashMap::new();

        let mut build_config = create_test_config("make build");
        build_config.dependencies = Some(vec!["test".to_string()]);

        configs.insert("test".to_string(), create_test_config("make test"));
        configs.insert("build".to_string(), build_config);

        let definitions = builder.build_tasks(configs).unwrap();

        assert_eq!(definitions.len(), 2);
        let build_def = &definitions["build"];
        assert_eq!(build_def.dependencies.len(), 1);
        assert_eq!(build_def.dependencies[0].name, "test");
    }

    #[test]
    fn test_circular_dependency_detection() {
        let temp_dir = TempDir::new().unwrap();
        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut configs = HashMap::new();

        let mut task1_config = create_test_config("echo task1");
        task1_config.dependencies = Some(vec!["task2".to_string()]);

        let mut task2_config = create_test_config("echo task2");
        task2_config.dependencies = Some(vec!["task1".to_string()]);

        configs.insert("task1".to_string(), task1_config);
        configs.insert("task2".to_string(), task2_config);

        let result = builder.build_tasks(configs);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));
    }

    #[test]
    fn test_missing_dependency_error() {
        let temp_dir = TempDir::new().unwrap();
        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut configs = HashMap::new();

        let mut build_config = create_test_config("make build");
        build_config.dependencies = Some(vec!["nonexistent".to_string()]);

        configs.insert("build".to_string(), build_config);

        let result = builder.build_tasks(configs);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_environment_variable_expansion() {
        let temp_dir = TempDir::new().unwrap();

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "expanded_value".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());

        let builder = TaskBuilder::new_with_env(temp_dir.path().to_path_buf(), env);

        let mut configs = HashMap::new();
        configs.insert(
            "test".to_string(),
            create_test_config("echo ${TEST_VAR} in ${HOME}"),
        );

        let definitions = builder.build_tasks(configs).unwrap();

        let definition = &definitions["test"];
        assert_eq!(
            definition.get_execution_content(),
            "echo expanded_value in /home/user"
        );
    }

    #[test]
    fn test_working_directory_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut configs = HashMap::new();
        let mut config = create_test_config("echo hello");
        config.working_dir = Some("subdir".to_string());

        configs.insert("test".to_string(), config);

        let definitions = builder.build_tasks(configs).unwrap();

        let definition = &definitions["test"];
        assert_eq!(
            definition.working_directory,
            sub_dir.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_security_validation() {
        let temp_dir = TempDir::new().unwrap();
        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut configs = HashMap::new();
        let mut config = create_test_config("echo hello");
        config.security = Some(SecurityConfig {
            restrict_disk: Some(true),
            restrict_network: Some(false),
            read_only_paths: Some(vec!["./readonly".to_string()]),
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: Some(vec!["example.com".to_string()]),
            infer_from_inputs_outputs: None,
        });

        configs.insert("test".to_string(), config);

        let definitions = builder.build_tasks(configs).unwrap();

        let definition = &definitions["test"];
        assert!(definition.security.is_some());
        let security = definition.security.as_ref().unwrap();
        assert!(security.restrict_disk);
        assert!(!security.restrict_network);
        assert_eq!(security.allowed_hosts, vec!["example.com"]);
    }

    #[test]
    fn test_invalid_shell_rejection() {
        let temp_dir = TempDir::new().unwrap();
        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut configs = HashMap::new();
        let mut config = create_test_config("echo hello");
        config.shell = Some("evil_shell".to_string());

        configs.insert("test".to_string(), config);

        let result = builder.build_tasks(configs);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not allowed"));
    }

    #[test]
    fn test_invalid_command_script_combination() {
        let temp_dir = TempDir::new().unwrap();
        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut configs = HashMap::new();
        let mut config = create_test_config("echo hello");
        config.script = Some("echo script".to_string()); // Both command and script

        configs.insert("test".to_string(), config);

        let result = builder.build_tasks(configs);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot have both"));
    }

    #[test]
    fn test_workspace_root_access() {
        let temp_dir = TempDir::new().unwrap();
        let builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        assert_eq!(builder.workspace_root(), temp_dir.path());
    }

    #[test]
    fn test_env_update() {
        let temp_dir = TempDir::new().unwrap();
        let mut builder = TaskBuilder::new(temp_dir.path().to_path_buf());

        let mut new_env = HashMap::new();
        new_env.insert("NEW_VAR".to_string(), "new_value".to_string());

        builder.update_env(new_env);

        let mut configs = HashMap::new();
        configs.insert("test".to_string(), create_test_config("echo ${NEW_VAR}"));

        let definitions = builder.build_tasks(configs).unwrap();
        let definition = &definitions["test"];
        assert_eq!(definition.get_execution_content(), "echo new_value");
    }
}
