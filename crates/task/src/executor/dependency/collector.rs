use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result, TaskDefinition};
use std::collections::{HashMap, HashSet};

/// Recursively collect all dependencies for a task
#[allow(dead_code)]
pub fn collect_dependencies(
    task_name: &str,
    all_tasks: &HashMap<String, TaskConfig>,
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

    let task_definition = all_tasks
        .get(task_name)
        .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

    let dependencies = task_definition.dependencies.clone().unwrap_or_default();

    // Validate and collect dependencies
    for dep_name in &dependencies {
        if !all_tasks.contains_key(dep_name) {
            return Err(Error::configuration(format!(
                "Dependency '{dep_name}' of task '{task_name}' not found"
            )));
        }

        collect_dependencies(dep_name, all_tasks, task_dependencies, visited, stack)?;
    }

    task_dependencies.insert(task_name.to_owned(), dependencies);
    visited.insert(task_name.to_owned());
    stack.remove(task_name);

    Ok(())
}

/// Recursively collect task dependencies from task definitions (Phase 3)
pub fn collect_dependencies_from_definitions(
    task_name: &str,
    all_tasks: &HashMap<String, TaskDefinition>,
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

    let task_definition = all_tasks
        .get(task_name)
        .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

    let dependencies = task_definition.dependency_names();

    // Validate and collect dependencies
    for dep_name in &dependencies {
        if !all_tasks.contains_key(dep_name) {
            return Err(Error::configuration(format!(
                "Dependency '{dep_name}' of task '{task_name}' not found"
            )));
        }

        collect_dependencies_from_definitions(
            dep_name,
            all_tasks,
            task_dependencies,
            visited,
            stack,
        )?;
    }

    task_dependencies.insert(task_name.to_owned(), dependencies);
    visited.insert(task_name.to_owned());
    stack.remove(task_name);

    Ok(())
}
