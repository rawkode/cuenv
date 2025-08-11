//! Dependency resolution and validation for task building
//!
//! This module handles task dependency resolution, circular dependency detection,
//! and caching of validation results.

use cuenv_core::{Error, ResolvedDependency, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::BuildContext;

/// Type alias for dependency validation cache
pub type DependencyValidationCache =
    Arc<std::sync::RwLock<HashMap<Vec<String>, std::result::Result<(), String>>>>;

/// Creates a new dependency validation cache
pub fn create_dependency_cache() -> DependencyValidationCache {
    Arc::new(std::sync::RwLock::new(HashMap::new()))
}

/// Resolve task dependencies and update the build context
pub fn resolve_dependencies(context: &mut BuildContext) -> Result<()> {
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
                    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
                        return Err(Error::configuration(format!(
                            "Invalid cross-package dependency '{dep_name}' in task '{task_name}'"
                        )));
                    }
                    ResolvedDependency::with_package(parts[1].to_string(), parts[0].to_string())
                } else {
                    // Local dependency
                    if !context.task_configs.contains_key(dep_name) {
                        return Err(Error::configuration(format!(
                            "Dependency '{dep_name}' of task '{task_name}' not found"
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
        context
            .dependency_graph
            .insert(task_name.clone(), dep_names);
    }

    Ok(())
}

/// Validate task dependencies for circular references with caching
pub fn validate_dependencies(
    context: &BuildContext,
    dependency_cache: &DependencyValidationCache,
) -> Result<()> {
    // Create a stable cache key from dependency graph
    let mut cache_key: Vec<String> = context
        .dependency_graph
        .iter()
        .map(|(k, v)| format!("{}:{}", k, v.join(",")))
        .collect();
    cache_key.sort(); // Ensure deterministic ordering

    // Check cache first
    if let Ok(cache) = dependency_cache.read() {
        if let Some(cached_result) = cache.get(&cache_key) {
            return match cached_result {
                Ok(()) => Ok(()),
                Err(err_msg) => Err(Error::configuration(err_msg.clone())),
            };
        }
    }

    // Perform validation if not cached
    let result = perform_dependency_validation(context);

    // Cache the result
    if let Ok(mut cache) = dependency_cache.write() {
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
fn perform_dependency_validation(context: &BuildContext) -> Result<()> {
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for task_name in context.dependency_graph.keys() {
        if !visited.contains(task_name) {
            detect_cycle(
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
                detect_cycle(dep_name, dependency_graph, visited, rec_stack)?;
            } else if rec_stack.contains(dep_name) {
                return Err(Error::configuration(format!(
                    "Circular dependency detected: task '{task_name}' depends on '{dep_name}' which creates a cycle"
                )));
            }
        }
    }

    rec_stack.remove(task_name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::TaskConfig;
    use cuenv_core::TaskDefinition;

    fn create_test_config(deps: Option<Vec<&str>>) -> TaskConfig {
        TaskConfig {
            description: Some("Test task".to_string()),
            command: Some("echo hello".to_string()),
            script: None,
            dependencies: deps.map(|d| d.iter().map(|s| s.to_string()).collect()),
            working_dir: None,
            shell: Some("sh".to_string()),
            inputs: None,
            outputs: None,
            security: None,
            cache: None,
            cache_key: None,
            cache_env: None,
            timeout: Some(30),
        }
    }

    fn create_test_definition(name: &str) -> TaskDefinition {
        TaskDefinition {
            name: name.to_string(),
            description: Some("Test task".to_string()),
            execution_mode: cuenv_core::TaskExecutionMode::Command {
                command: "echo hello".to_string(),
            },
            dependencies: Vec::new(),
            working_directory: std::path::PathBuf::from("."),
            shell: "sh".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            security: None,
            cache: cuenv_core::TaskCache::default(),
            timeout: std::time::Duration::from_secs(30),
        }
    }

    #[test]
    fn test_resolve_dependencies_success() {
        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        context
            .task_configs
            .insert("test".to_string(), create_test_config(None));
        context
            .task_configs
            .insert("build".to_string(), create_test_config(Some(vec!["test"])));

        context
            .task_definitions
            .insert("test".to_string(), create_test_definition("test"));
        context
            .task_definitions
            .insert("build".to_string(), create_test_definition("build"));

        let result = resolve_dependencies(&mut context);
        assert!(result.is_ok());

        let build_def = &context.task_definitions["build"];
        assert_eq!(build_def.dependencies.len(), 1);
        assert_eq!(build_def.dependencies[0].name, "test");
    }

    #[test]
    fn test_resolve_missing_dependency() {
        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        context.task_configs.insert(
            "build".to_string(),
            create_test_config(Some(vec!["nonexistent"])),
        );
        context
            .task_definitions
            .insert("build".to_string(), create_test_definition("build"));

        let result = resolve_dependencies(&mut context);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let cache = create_dependency_cache();
        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        // Create circular dependency: task1 -> task2 -> task1
        context
            .dependency_graph
            .insert("task1".to_string(), vec!["task2".to_string()]);
        context
            .dependency_graph
            .insert("task2".to_string(), vec!["task1".to_string()]);

        let result = validate_dependencies(&context, &cache);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));
    }

    #[test]
    fn test_valid_dependency_chain() {
        let cache = create_dependency_cache();
        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        // Create valid chain: task1 -> task2 -> task3
        context
            .dependency_graph
            .insert("task1".to_string(), vec!["task2".to_string()]);
        context
            .dependency_graph
            .insert("task2".to_string(), vec!["task3".to_string()]);
        context.dependency_graph.insert("task3".to_string(), vec![]);

        let result = validate_dependencies(&context, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cross_package_dependency_parsing() {
        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        context.task_configs.insert(
            "build".to_string(),
            create_test_config(Some(vec!["pkg:task"])),
        );
        context
            .task_definitions
            .insert("build".to_string(), create_test_definition("build"));

        let result = resolve_dependencies(&mut context);
        assert!(result.is_ok());

        let build_def = &context.task_definitions["build"];
        assert_eq!(build_def.dependencies.len(), 1);
        assert_eq!(build_def.dependencies[0].name, "task");
    }

    #[test]
    fn test_invalid_cross_package_format() {
        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        context.task_configs.insert(
            "build".to_string(),
            create_test_config(Some(vec!["invalid:"])),
        );
        context
            .task_definitions
            .insert("build".to_string(), create_test_definition("build"));

        let result = resolve_dependencies(&mut context);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid cross-package dependency"));
    }
}
