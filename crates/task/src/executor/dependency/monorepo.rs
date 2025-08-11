use crate::MonorepoTaskRegistry;
use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};
use std::collections::{HashMap, HashSet};

/// Recursively collect dependencies for monorepo tasks
pub fn collect_monorepo_dependencies(
    task_name: &str,
    registry: &MonorepoTaskRegistry,
    all_tasks: &mut HashMap<String, TaskConfig>,
    task_dependencies: &mut HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    stack: &mut HashSet<String>,
) -> Result<()> {
    // Check for circular dependencies
    if stack.contains(task_name) {
        return Err(Error::configuration(format!(
            "Circular dependency detected involving task '{task_name}'"
        )));
    }

    if visited.contains(task_name) {
        return Ok(());
    }

    stack.insert(task_name.to_owned());

    let task = registry
        .get_task(task_name)
        .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

    // Add task config to all_tasks
    all_tasks.insert(task_name.to_owned(), task.config.clone());

    let mut dependencies = Vec::new();

    // Process dependencies, resolving cross-package references
    if let Some(ref deps) = task.config.dependencies {
        for dep in deps {
            // Check if this is a cross-package reference
            let full_dep_name = if dep.contains(':') {
                // Already a full cross-package reference
                dep.clone()
            } else {
                // Local task reference, add package prefix
                format!("{}:{}", task.package_name, dep)
            };

            // Validate dependency exists
            if registry.get_task(&full_dep_name).is_none() {
                return Err(Error::configuration(format!(
                    "Dependency '{full_dep_name}' of task '{task_name}' not found"
                )));
            }

            dependencies.push(full_dep_name.clone());

            // Recursively collect dependencies
            collect_monorepo_dependencies(
                &full_dep_name,
                registry,
                all_tasks,
                task_dependencies,
                visited,
                stack,
            )?;
        }
    }

    task_dependencies.insert(task_name.to_owned(), dependencies);
    visited.insert(task_name.to_owned());
    stack.remove(task_name);

    Ok(())
}
