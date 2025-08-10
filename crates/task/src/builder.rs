//! Task Builder for Phase 3 architecture
//!
//! This module provides the TaskBuilder that separates task building from execution.
//! It takes raw TaskConfig objects and produces validated, immutable TaskDefinition
//! objects ready for execution.

use crate::definition::{ResolvedDependency, TaskDefinition, TaskExecutionMode, TaskSecurity};
use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Task builder that validates and builds task definitions from configurations
#[derive(Clone)]
pub struct TaskBuilder {
    /// Workspace root directory
    workspace_root: PathBuf,
    /// Global environment variables for expansion
    global_env: HashMap<String, String>,
    /// Cached dependency validation results
    dependency_cache: Arc<std::sync::RwLock<HashMap<Vec<String>, std::result::Result<(), String>>>>,
}

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
            dependency_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
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
        self.validate_task_configs(&task_configs)?;

        // Step 2: Build initial task definitions
        for (name, config) in &task_configs {
            let mut definition = TaskDefinition::try_from(config.clone())?;
            definition.name = name.clone();
            context.task_definitions.insert(name.clone(), definition);
        }

        // Step 3: Resolve dependencies
        self.resolve_dependencies(&mut context)?;

        // Step 4: Validate dependency graph for cycles
        self.validate_dependencies(&context)?;

        // Step 5: Expand environment variables
        self.expand_environment_variables(&mut context)?;

        // Step 6: Resolve working directories  
        self.resolve_working_directories(&mut context)?;

        // Step 7: Validate security configurations
        self.validate_security_configs(&mut context)?;

        Ok(context.task_definitions)
    }

    /// Validate basic task configurations
    fn validate_task_configs(&self, task_configs: &HashMap<String, TaskConfig>) -> Result<()> {
        for (name, config) in task_configs {
            // Validate task name
            if name.is_empty() {
                return Err(Error::configuration("Task name cannot be empty".to_string()));
            }

            // Validate command/script exclusivity
            match (&config.command, &config.script) {
                (Some(_), Some(_)) => {
                    return Err(Error::configuration(format!(
                        "Task '{}' cannot have both 'command' and 'script' defined",
                        name
                    )));
                }
                (None, None) => {
                    return Err(Error::configuration(format!(
                        "Task '{}' must have either 'command' or 'script' defined",
                        name
                    )));
                }
                _ => {} // Valid
            }

            // Validate shell
            if let Some(shell) = &config.shell {
                self.validate_shell(shell)?;
            }

            // Validate timeout
            if let Some(timeout) = config.timeout {
                if timeout == 0 {
                    return Err(Error::configuration(format!(
                        "Task '{}' timeout must be greater than 0",
                        name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Validate shell command
    fn validate_shell(&self, shell: &str) -> Result<()> {
        const ALLOWED_SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "pwsh", "powershell"];
        
        if !ALLOWED_SHELLS.contains(&shell) {
            return Err(Error::configuration(format!(
                "Shell '{}' is not allowed. Allowed shells: {}",
                shell,
                ALLOWED_SHELLS.join(", ")
            )));
        }

        // Check if shell is available on the system (best effort)
        if let Err(_) = std::process::Command::new("which").arg(shell).output() {
            tracing::warn!("Shell '{}' may not be available on this system", shell);
        }
        
        Ok(())
    }

    /// Resolve task dependencies
    fn resolve_dependencies(&self, context: &mut BuildContext) -> Result<()> {
        for (task_name, config) in &context.task_configs {
            let mut resolved_deps = Vec::new();
            let mut dep_names = Vec::new();

            if let Some(dependencies) = &config.dependencies {
                for dep_name in dependencies {
                    // For now, all dependencies are local (no cross-package support)
                    // Future enhancement would parse "package:task" format
                    let resolved_dep = if dep_name.contains(':') {
                        // Cross-package dependency (future feature)
                        let parts: Vec<&str> = dep_name.splitn(2, ':').collect();
                        if parts.len() != 2 {
                            return Err(Error::configuration(format!(
                                "Invalid cross-package dependency '{}' in task '{}'",
                                dep_name, task_name
                            )));
                        }
                        ResolvedDependency::with_package(
                            parts[1].to_string(),
                            parts[0].to_string(),
                        )
                    } else {
                        // Local dependency
                        if !context.task_configs.contains_key(dep_name) {
                            return Err(Error::configuration(format!(
                                "Dependency '{}' of task '{}' not found",
                                dep_name, task_name
                            )));
                        }
                        ResolvedDependency::new(dep_name.clone())
                    };

                    resolved_deps.push(resolved_dep);
                    dep_names.push(dep_name.clone());
                }
            }

            // Update task definition with resolved dependencies
            if let Some(definition) = context.task_definitions.get_mut(task_name) {
                definition.dependencies = resolved_deps;
            }

            // Update dependency graph
            context.dependency_graph.insert(task_name.clone(), dep_names);
        }

        Ok(())
    }

    /// Validate task dependencies for circular references with caching
    fn validate_dependencies(&self, context: &BuildContext) -> Result<()> {
        // Create a stable cache key from dependency graph
        let mut cache_key: Vec<String> = context.dependency_graph
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v.join(",")))
            .collect();
        cache_key.sort(); // Ensure deterministic ordering

        // Check cache first
        if let Ok(cache) = self.dependency_cache.read() {
            if let Some(cached_result) = cache.get(&cache_key) {
                return match cached_result {
                    Ok(()) => Ok(()),
                    Err(err_msg) => Err(Error::configuration(err_msg.clone())),
                };
            }
        }

        // Perform validation if not cached
        let result = self.perform_dependency_validation(context);
        
        // Cache the result
        if let Ok(mut cache) = self.dependency_cache.write() {
            match &result {
                Ok(()) => {
                    cache.insert(cache_key, Ok(()));
                }
                Err(err) => {
                    cache.insert(cache_key, Err(err.to_string()));
                }
            }
        }

        result
    }

    /// Perform the actual dependency validation
    fn perform_dependency_validation(&self, context: &BuildContext) -> Result<()> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for task_name in context.dependency_graph.keys() {
            if !visited.contains(task_name) {
                self.detect_cycle(
                    task_name,
                    &context.dependency_graph,
                    &mut visited,
                    &mut rec_stack,
                )?;
            }
        }

        Ok(())
    }

    /// Detect circular dependencies using DFS
    fn detect_cycle(
        &self,
        task_name: &str,
        dependency_graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> Result<()> {
        visited.insert(task_name.to_string());
        rec_stack.insert(task_name.to_string());

        if let Some(dependencies) = dependency_graph.get(task_name) {
            for dep_name in dependencies {
                if !visited.contains(dep_name) {
                    self.detect_cycle(dep_name, dependency_graph, visited, rec_stack)?;
                } else if rec_stack.contains(dep_name) {
                    return Err(Error::configuration(format!(
                        "Circular dependency detected: task '{}' depends on '{}' which creates a cycle",
                        task_name, dep_name
                    )));
                }
            }
        }

        rec_stack.remove(task_name);
        Ok(())
    }

    /// Expand environment variables in commands and scripts
    fn expand_environment_variables(&self, context: &mut BuildContext) -> Result<()> {
        for (_, definition) in &mut context.task_definitions {
            // Expand environment variables in execution content
            match &mut definition.execution_mode {
                TaskExecutionMode::Command { command } => {
                    *command = self.expand_env_vars(command)?;
                }
                TaskExecutionMode::Script { content } => {
                    *content = self.expand_env_vars(content)?;
                }
            }
        }

        Ok(())
    }

    /// Expand environment variables in a string using ${VAR} syntax
    fn expand_env_vars(&self, input: &str) -> Result<String> {
        let mut result = input.to_string();
        let mut start = 0;

        while let Some(pos) = result[start..].find("${") {
            let abs_pos = start + pos;
            
            if let Some(end_pos) = result[abs_pos + 2..].find('}') {
                let var_name = &result[abs_pos + 2..abs_pos + 2 + end_pos];
                
                // Look up the variable in global environment
                let var_value = self.global_env
                    .get(var_name)
                    .map(|s| s.as_str())
                    .unwrap_or("");

                // Replace ${VAR} with the value
                result.replace_range(abs_pos..abs_pos + 3 + end_pos, var_value);
                start = abs_pos + var_value.len();
            } else {
                // No closing brace found, skip this occurrence
                start = abs_pos + 2;
            }
        }

        Ok(result)
    }

    /// Resolve working directories to absolute paths
    fn resolve_working_directories(&self, context: &mut BuildContext) -> Result<()> {
        for (_, definition) in &mut context.task_definitions {
            // If working_directory is relative, make it relative to workspace_root
            if !definition.working_directory.is_absolute() {
                definition.working_directory = self
                    .workspace_root
                    .join(&definition.working_directory)
                    .canonicalize()
                    .map_err(|e| {
                        Error::configuration(format!(
                            "Failed to resolve working directory '{}' for task '{}': {}",
                            definition.working_directory.display(),
                            definition.name,
                            e
                        ))
                    })?;
            }
        }

        Ok(())
    }

    /// Validate security configurations
    fn validate_security_configs(&self, context: &mut BuildContext) -> Result<()> {
        for (task_name, definition) in &mut context.task_definitions {
            if let Some(security) = &mut definition.security {
                // Validate and resolve paths
                self.resolve_security_paths(task_name, security)?;

                // Validate hosts
                self.validate_security_hosts(task_name, security)?;
            }
        }

        Ok(())
    }

    /// Resolve security paths to absolute paths and validate them
    fn resolve_security_paths(&self, task_name: &str, security: &mut TaskSecurity) -> Result<()> {
        let workspace_root = &self.workspace_root;

        // Helper to resolve and validate paths
        let resolve_paths = |paths: &mut Vec<PathBuf>| -> Result<()> {
            for path in paths.iter_mut() {
                if !path.is_absolute() {
                    *path = workspace_root.join(&path);
                }

                // Validate that the path is within reasonable bounds (security check)
                if let Ok(canonical) = path.canonicalize() {
                    *path = canonical;
                } else {
                    // Path doesn't exist - that's OK for security restrictions
                    // but ensure it's at least under workspace
                    if let Ok(canonical_workspace) = workspace_root.canonicalize() {
                        if !path.starts_with(&canonical_workspace) {
                            return Err(Error::configuration(format!(
                                "Security path '{}' in task '{}' must be within workspace",
                                path.display(),
                                task_name
                            )));
                        }
                    }
                }
            }
            Ok(())
        };

        resolve_paths(&mut security.read_only_paths)?;
        resolve_paths(&mut security.read_write_paths)?;
        resolve_paths(&mut security.deny_paths)?;

        Ok(())
    }

    /// Validate security host configurations
    fn validate_security_hosts(&self, task_name: &str, security: &TaskSecurity) -> Result<()> {
        for host in &security.allowed_hosts {
            if host.is_empty() {
                return Err(Error::configuration(format!(
                    "Empty host in allowed_hosts for task '{}'",
                    task_name
                )));
            }

            // Basic validation - ensure it's not just whitespace and has reasonable format
            if host.trim() != host || host.contains(' ') {
                return Err(Error::configuration(format!(
                    "Invalid host '{}' in allowed_hosts for task '{}'. Hosts cannot contain spaces",
                    host, task_name
                )));
            }
        }

        Ok(())
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
        assert!(!build_def.dependencies[0].is_cross_package());
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
        assert!(result.unwrap_err().to_string().contains("Circular dependency"));
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
        assert_eq!(definition.get_execution_content(), "echo expanded_value in /home/user");
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
        assert_eq!(definition.working_directory, sub_dir.canonicalize().unwrap());
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
        assert!(definition.has_security_restrictions());
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
}