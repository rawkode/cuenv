use crate::core::errors::{Error, Result};
use crate::task::cross_package::{parse_reference, CrossPackageReference};
use crate::task::registry::MonorepoTaskRegistry;
use crate::task::staging::{DependencyStager, StagedDependency};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

/// Task executor that handles cross-package dependencies
pub struct TaskExecutor {
    /// Registry of all tasks in the monorepo
    registry: MonorepoTaskRegistry,
    /// Cache of executed tasks
    executed: HashSet<String>,
    /// Staging strategy for dependencies
    use_staging: bool,
}

impl TaskExecutor {
    /// Create a new task executor with a registry
    pub fn new(registry: MonorepoTaskRegistry) -> Self {
        Self {
            registry,
            executed: HashSet::new(),
            use_staging: true,
        }
    }

    /// Create a task executor without staging (for testing)
    pub fn without_staging(registry: MonorepoTaskRegistry) -> Self {
        Self {
            registry,
            executed: HashSet::new(),
            use_staging: false,
        }
    }

    /// Execute a task by its full name (e.g., "projects:frontend:build")
    pub fn execute(&mut self, task_name: &str) -> Result<()> {
        // Check for circular dependencies
        let mut visited = HashSet::new();
        self.check_circular_dependencies(task_name, &mut visited)?;

        // Execute the task and its dependencies
        self.execute_with_deps(task_name)
    }

    /// Check for circular dependencies
    fn check_circular_dependencies(
        &self,
        task_name: &str,
        visited: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(task_name) {
            return Err(Error::configuration(format!(
                "Circular dependency detected: {}",
                task_name
            )));
        }

        visited.insert(task_name.to_string());

        // Get the task
        let task = self
            .registry
            .get_task(task_name)
            .ok_or_else(|| Error::configuration(format!("Task '{}' not found", task_name)))?;

        // Check dependencies recursively
        if let Some(ref deps) = task.config.dependencies {
            for dep in deps {
                let dep_ref = parse_reference(dep)?;
                let full_dep_name = self.resolve_full_task_name(&dep_ref, &task.package_name)?;
                self.check_circular_dependencies(&full_dep_name, visited)?;
            }
        }

        visited.remove(task_name);
        Ok(())
    }

    /// Execute a task and its dependencies
    fn execute_with_deps(&mut self, task_name: &str) -> Result<()> {
        // Skip if already executed
        if self.executed.contains(task_name) {
            println!("Task '{}' already executed, skipping", task_name);
            return Ok(());
        }

        // Get the task
        let task = self
            .registry
            .get_task(task_name)
            .ok_or_else(|| Error::configuration(format!("Task '{}' not found", task_name)))?
            .clone(); // Clone to avoid borrow issues

        println!("Executing task: {}", task_name);

        // Execute dependencies first
        if let Some(ref deps) = task.config.dependencies {
            for dep in deps {
                let dep_ref = parse_reference(dep)?;
                let full_dep_name = self.resolve_full_task_name(&dep_ref, &task.package_name)?;
                self.execute_with_deps(&full_dep_name)?;
            }
        }

        // Stage inputs if needed
        let staging_env = if self.use_staging && task.config.inputs.is_some() {
            Some(self.stage_task_inputs(&task.full_name)?)
        } else {
            None
        };

        // Execute the task command
        if let Some(ref command) = task.config.command {
            self.run_command(command, &task.package_path, staging_env.as_ref())?;
        } else if let Some(ref script) = task.config.script {
            self.run_script(script, &task.package_path, staging_env.as_ref())?;
        } else {
            return Err(Error::configuration(format!(
                "Task '{}' has no command or script",
                task_name
            )));
        }

        // Mark as executed
        self.executed.insert(task_name.to_string());
        println!("Task '{}' completed successfully", task_name);

        Ok(())
    }

    /// Resolve a cross-package reference to a full task name
    fn resolve_full_task_name(
        &self,
        reference: &CrossPackageReference,
        current_package: &str,
    ) -> Result<String> {
        match reference {
            CrossPackageReference::LocalTask { task } => {
                Ok(format!("{}:{}", current_package, task))
            }
            CrossPackageReference::PackageTask { package, task } => {
                Ok(format!("{}:{}", package, task))
            }
            CrossPackageReference::PackageTaskOutput { package, task, .. } => {
                Ok(format!("{}:{}", package, task))
            }
        }
    }

    /// Stage all inputs for a task
    fn stage_task_inputs(&self, task_name: &str) -> Result<HashMap<String, String>> {
        let task = self
            .registry
            .get_task(task_name)
            .ok_or_else(|| Error::configuration(format!("Task '{}' not found", task_name)))?;

        let mut stager = DependencyStager::new()?;
        
        if let Some(ref inputs) = task.config.inputs {
            for input in inputs {
                let input_ref = parse_reference(input)?;
                
                match input_ref {
                    CrossPackageReference::PackageTaskOutput { package, task, output } => {
                        let full_task_name = format!("{}:{}", package, task);
                        let output_path = self.registry.resolve_task_output(&full_task_name, &output)?;
                        
                        let staged_dep = StagedDependency {
                            name: format!("{}:{}:{}", package, task, output),
                            source_path: output_path,
                            target_name: Some(output.clone()),
                        };
                        
                        stager.stage_dependency(&staged_dep)?;
                    }
                    _ => {
                        // For non-output references, we can't stage them
                        // This might be a dependency without specific output
                        continue;
                    }
                }
            }
        }

        Ok(stager.get_environment_variables())
    }

    /// Run a command in a specific directory with optional environment
    fn run_command(
        &self,
        command: &str,
        working_dir: &Path,
        env_vars: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        println!("  Running: {} in {}", command, working_dir.display());

        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(&["/C", command]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(&["-c", command]);
            cmd
        };

        cmd.current_dir(working_dir);

        // Add staging environment variables if present
        if let Some(env) = env_vars {
            for (key, value) in env {
                cmd.env(key, value);
                println!("  Environment: {}={}", key, value);
            }
        }

        let output = cmd.output().map_err(|e| {
            Error::command_execution(
                command,
                vec![],
                e.to_string(),
                None,
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::configuration(format!(
                "Command '{}' failed: {}",
                command, stderr
            )));
        }

        Ok(())
    }

    /// Run a script file in a specific directory with optional environment
    fn run_script(
        &self,
        script_path: &str,
        working_dir: &Path,
        env_vars: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        let full_path = working_dir.join(script_path);
        
        if !full_path.exists() {
            return Err(Error::configuration(format!(
                "Script '{}' not found at {}",
                script_path,
                full_path.display()
            )));
        }

        println!("  Running script: {}", full_path.display());

        let mut cmd = if cfg!(target_os = "windows") {
            Command::new(&full_path)
        } else {
            let mut cmd = Command::new("sh");
            cmd.arg(&full_path);
            cmd
        };

        cmd.current_dir(working_dir);

        // Add staging environment variables if present
        if let Some(env) = env_vars {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        let output = cmd.output().map_err(|e| {
            Error::command_execution(
                &full_path.display().to_string(),
                vec![],
                e.to_string(),
                None,
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::configuration(format!(
                "Script '{}' failed: {}",
                script_path, stderr
            )));
        }

        Ok(())
    }

    /// Get a topologically sorted list of tasks to execute
    pub fn get_execution_order(&self, task_name: &str) -> Result<Vec<String>> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        self.topological_sort(task_name, &mut visited, &mut order)?;
        Ok(order)
    }

    /// Perform topological sort for task execution order
    fn topological_sort(
        &self,
        task_name: &str,
        visited: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<()> {
        if visited.contains(task_name) {
            return Ok(());
        }

        visited.insert(task_name.to_string());

        // Get the task
        let task = self
            .registry
            .get_task(task_name)
            .ok_or_else(|| Error::configuration(format!("Task '{}' not found", task_name)))?;

        // Visit dependencies first
        if let Some(ref deps) = task.config.dependencies {
            for dep in deps {
                let dep_ref = parse_reference(dep)?;
                let full_dep_name = self.resolve_full_task_name(&dep_ref, &task.package_name)?;
                self.topological_sort(&full_dep_name, visited, order)?;
            }
        }

        order.push(task_name.to_string());
        Ok(())
    }

    /// Check if a task has been executed
    pub fn is_executed(&self, task_name: &str) -> bool {
        self.executed.contains(task_name)
    }

    /// Clear execution cache
    pub fn clear_cache(&mut self) {
        self.executed.clear();
    }

    /// Get the registry
    pub fn registry(&self) -> &MonorepoTaskRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TaskConfig;
    use crate::discovery::DiscoveredPackage;
    use std::path::PathBuf;

    fn create_test_registry() -> MonorepoTaskRegistry {
        let mut packages = Vec::new();

        // Create a mock package with tasks
        let mut parse_result = crate::config::ParseResult::default();
        
        let build_task = TaskConfig {
            command: Some("echo 'building'".to_string()),
            outputs: Some(vec!["dist".to_string()]),
            ..Default::default()
        };
        
        let test_task = TaskConfig {
            command: Some("echo 'testing'".to_string()),
            dependencies: Some(vec!["build".to_string()]),
            ..Default::default()
        };

        parse_result.tasks.insert("build".to_string(), build_task);
        parse_result.tasks.insert("test".to_string(), test_task);

        let package = DiscoveredPackage {
            name: "test".to_string(),
            path: PathBuf::from("/test"),
            parse_result: Some(parse_result),
        };

        packages.push(package);

        MonorepoTaskRegistry::from_packages(packages).unwrap()
    }

    #[test]
    fn test_execution_order() {
        let registry = create_test_registry();
        let executor = TaskExecutor::new(registry);

        let order = executor.get_execution_order("test:test").unwrap();
        assert_eq!(order, vec!["test:build", "test:test"]);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut packages = Vec::new();

        // Create packages with circular dependency
        let mut parse_result = crate::config::ParseResult::default();
        
        let task_a = TaskConfig {
            command: Some("echo 'a'".to_string()),
            dependencies: Some(vec!["b".to_string()]),
            ..Default::default()
        };
        
        let task_b = TaskConfig {
            command: Some("echo 'b'".to_string()),
            dependencies: Some(vec!["a".to_string()]),
            ..Default::default()
        };

        parse_result.tasks.insert("a".to_string(), task_a);
        parse_result.tasks.insert("b".to_string(), task_b);

        let package = DiscoveredPackage {
            name: "test".to_string(),
            path: PathBuf::from("/test"),
            parse_result: Some(parse_result),
        };

        packages.push(package);

        let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();
        let mut executor = TaskExecutor::new(registry);

        let result = executor.execute("test:a");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular dependency"));
    }
}