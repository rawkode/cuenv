use crate::config::TaskConfig;
use crate::core::errors::{Error, Result};
use crate::discovery::DiscoveredPackage;
use crate::task::cross_package::{parse_reference, CrossPackageReference};
use std::collections::HashMap;
use std::path::PathBuf;

/// A task registered in the monorepo registry
#[derive(Debug, Clone)]
pub struct RegisteredTask {
    /// Full task name (e.g., "projects:frontend:build")
    pub full_name: String,
    /// Package name (e.g., "projects:frontend")
    pub package_name: String,
    /// Task name within the package (e.g., "build")
    pub task_name: String,
    /// Absolute path to the package directory
    pub package_path: PathBuf,
    /// Task configuration
    pub config: TaskConfig,
}

/// Registry of all tasks across all packages in a monorepo
pub struct MonorepoTaskRegistry {
    /// Map from full task name to registered task
    tasks: HashMap<String, RegisteredTask>,
    /// Map from package name to package path
    package_paths: HashMap<String, PathBuf>,
    /// Cached task configs for TaskSource trait
    task_configs: HashMap<String, TaskConfig>,
    /// Empty env vars for TaskSource trait
    empty_env_vars: HashMap<String, String>,
}

impl MonorepoTaskRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            package_paths: HashMap::new(),
            task_configs: HashMap::new(),
            empty_env_vars: HashMap::new(),
        }
    }

    /// Create a registry from discovered packages
    pub fn from_packages(packages: Vec<DiscoveredPackage>) -> Result<Self> {
        let mut registry = Self::new();

        for package in packages {
            if let Some(parse_result) = package.parse_result {
                // Register package path
                registry
                    .package_paths
                    .insert(package.name.clone(), package.path.clone());

                // Register all tasks from this package
                for (task_name, task_config) in parse_result.tasks {
                    let full_name = if package.name == "root" {
                        format!("root:{}", task_name)
                    } else {
                        format!("{}:{}", package.name, task_name)
                    };

                    let registered_task = RegisteredTask {
                        full_name: full_name.clone(),
                        package_name: package.name.clone(),
                        task_name: task_name.clone(),
                        package_path: package.path.clone(),
                        config: task_config.clone(),
                    };

                    registry.tasks.insert(full_name.clone(), registered_task);
                    registry.task_configs.insert(full_name, task_config);
                }
            }
        }

        Ok(registry)
    }

    /// Get a task by its full name
    pub fn get_task(&self, full_name: &str) -> Option<&RegisteredTask> {
        self.tasks.get(full_name)
    }

    /// Get all tasks for a specific package
    pub fn get_tasks_by_package(&self, package_name: &str) -> Vec<&RegisteredTask> {
        self.tasks
            .values()
            .filter(|task| task.package_name == package_name)
            .collect()
    }

    /// List all tasks in the registry
    pub fn list_all_tasks(&self) -> Vec<(String, Option<String>)> {
        self.tasks
            .values()
            .map(|task| (task.full_name.clone(), task.config.description.clone()))
            .collect()
    }

    /// Get the package path for a package name
    pub fn get_package_path(&self, package_name: &str) -> Option<&PathBuf> {
        self.package_paths.get(package_name)
    }

    /// Resolve a task output to its filesystem path
    pub fn resolve_task_output(&self, task_ref: &str, output_name: &str) -> Result<PathBuf> {
        // Get the task
        let task = self
            .get_task(task_ref)
            .ok_or_else(|| Error::configuration(format!("Task '{}' not found", task_ref)))?;

        // Check if the task declares this output
        if let Some(ref outputs) = task.config.outputs {
            if !outputs
                .iter()
                .any(|o| o == output_name || o.ends_with(output_name))
            {
                return Err(Error::configuration(format!(
                    "Task '{}' does not declare output '{}'",
                    task_ref, output_name
                )));
            }
        } else {
            return Err(Error::configuration(format!(
                "Task '{}' does not declare any outputs",
                task_ref
            )));
        }

        // Construct the output path
        let output_path = task.package_path.join(output_name);

        // Check if the output exists
        if !output_path.exists() {
            return Err(Error::configuration(format!(
                "Output '{}' for task '{}' does not exist at {}",
                output_name,
                task_ref,
                output_path.display()
            )));
        }

        Ok(output_path)
    }

    /// Validate that all task dependencies exist
    pub fn validate_all_dependencies(&self) -> Result<()> {
        for (task_name, task) in &self.tasks {
            if let Some(ref deps) = task.config.dependencies {
                for dep in deps {
                    // Parse the dependency reference
                    let dep_ref = parse_reference(dep)?;

                    // For cross-package dependencies, check if the task exists
                    if dep_ref.is_cross_package() {
                        let full_dep_name = match dep_ref {
                            CrossPackageReference::PackageTask { package, task } => {
                                format!("{}:{}", package, task)
                            }
                            CrossPackageReference::PackageTaskOutput { package, task, .. } => {
                                format!("{}:{}", package, task)
                            }
                            _ => dep.clone(),
                        };

                        if !self.tasks.contains_key(&full_dep_name) {
                            return Err(Error::configuration(format!(
                                "Task '{}' depends on non-existent task '{}'",
                                task_name, full_dep_name
                            )));
                        }
                    } else {
                        // For local dependencies, check in the same package
                        let local_task_name = format!("{}:{}", task.package_name, dep);
                        if !self.tasks.contains_key(&local_task_name) {
                            return Err(Error::configuration(format!(
                                "Task '{}' depends on non-existent local task '{}'",
                                task_name, dep
                            )));
                        }
                    }
                }
            }

            // Validate inputs reference existing outputs
            if let Some(ref inputs) = task.config.inputs {
                for input in inputs {
                    // Parse input reference
                    let input_ref = parse_reference(input)?;

                    if let Some(output) = input_ref.output() {
                        // This is a reference to a specific output
                        let task_ref = match &input_ref {
                            CrossPackageReference::PackageTaskOutput { package, task, .. } => {
                                format!("{}:{}", package, task)
                            }
                            _ => continue,
                        };

                        // Check if we can resolve this output (don't fail if it doesn't exist yet)
                        if let Some(ref_task) = self.get_task(&task_ref) {
                            if let Some(ref outputs) = ref_task.config.outputs {
                                if !outputs.iter().any(|o| o == output) {
                                    return Err(Error::configuration(format!(
                                        "Task '{}' references non-existent output '{}' from task '{}'",
                                        task_name, output, task_ref
                                    )));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get all tasks that depend on a given task
    pub fn get_dependents(&self, task_name: &str) -> Vec<&RegisteredTask> {
        self.tasks
            .values()
            .filter(|registered_task| {
                if let Some(ref deps) = registered_task.config.dependencies {
                    deps.iter().any(|dep| {
                        // Parse the dependency and check if it matches
                        if let Ok(dep_ref) = parse_reference(dep) {
                            match dep_ref {
                                CrossPackageReference::LocalTask { task } => {
                                    // Local dependency - need to match within same package
                                    let local_full_name =
                                        format!("{}:{}", registered_task.package_name, task);
                                    local_full_name == task_name
                                }
                                CrossPackageReference::PackageTask { package, task } => {
                                    let full_name = format!("{}:{}", package, task);
                                    full_name == task_name
                                }
                                CrossPackageReference::PackageTaskOutput {
                                    package, task, ..
                                } => {
                                    let full_name = format!("{}:{}", package, task);
                                    full_name == task_name
                                }
                            }
                        } else {
                            false
                        }
                    })
                } else {
                    false
                }
            })
            .collect()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get the number of registered tasks
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Get the number of registered packages
    pub fn package_count(&self) -> usize {
        self.package_paths.len()
    }
}

impl Default for MonorepoTaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = MonorepoTaskRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.task_count(), 0);
        assert_eq!(registry.package_count(), 0);
    }

    #[test]
    fn test_task_registration() {
        let mut registry = MonorepoTaskRegistry::new();

        let task = RegisteredTask {
            full_name: "test:task".to_string(),
            package_name: "test".to_string(),
            task_name: "task".to_string(),
            package_path: PathBuf::from("/test"),
            config: TaskConfig {
                command: Some("echo test".to_string()),
                description: Some("Test task".to_string()),
                ..Default::default()
            },
        };

        registry.tasks.insert("test:task".to_string(), task);
        registry
            .package_paths
            .insert("test".to_string(), PathBuf::from("/test"));

        assert_eq!(registry.task_count(), 1);
        assert_eq!(registry.package_count(), 1);
        assert!(registry.get_task("test:task").is_some());
    }
}
