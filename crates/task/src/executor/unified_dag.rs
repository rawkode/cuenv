//! Unified Task DAG system for consolidating all execution paths
//!
//! This module provides a single, consistent way to build and execute task graphs
//! regardless of how tasks are invoked (direct execution, dependency resolution, etc.).
//! It ensures proper ordering preservation and eliminates code duplication.

use cuenv_config::{TaskCollection, TaskConfig, TaskNode};
use cuenv_core::{Result, TaskDefinition};
use indexmap::IndexMap;
use std::collections::HashMap;

use super::strategies::{FlattenedTask, GroupExecutionStrategy, GroupStrategy, SequentialStrategy};

/// A unified DAG builder that consolidates all task execution paths
#[derive(Debug, Clone)]
pub struct UnifiedTaskDAG {
    /// All available task configurations (flattened)
    task_configs: HashMap<String, TaskConfig>,
    /// Task nodes preserving hierarchical structure and insertion order
    task_nodes: IndexMap<String, TaskNode>,
    /// Task definitions built from configs
    task_definitions: HashMap<String, TaskDefinition>,
    /// The flattened execution graph
    execution_graph: Vec<FlattenedTask>,
    /// Task dependencies for topological sorting
    dependencies: HashMap<String, Vec<String>>,
}

/// Builder for creating a UnifiedTaskDAG
#[derive(Debug)]
pub struct DAGBuilder {
    task_configs: HashMap<String, TaskConfig>,
    task_nodes: IndexMap<String, TaskNode>,
    task_definitions: HashMap<String, TaskDefinition>,
}

impl DAGBuilder {
    /// Create a new DAG builder
    pub fn new() -> Self {
        Self {
            task_configs: HashMap::new(),
            task_nodes: IndexMap::new(),
            task_definitions: HashMap::new(),
        }
    }

    /// Add task configurations to the builder
    pub fn with_task_configs(mut self, configs: HashMap<String, TaskConfig>) -> Self {
        self.task_configs = configs;
        self
    }

    /// Add task nodes to the builder
    pub fn with_task_nodes(mut self, nodes: IndexMap<String, TaskNode>) -> Self {
        self.task_nodes = nodes;
        self
    }

    /// Add task definitions to the builder
    pub fn with_task_definitions(mut self, definitions: HashMap<String, TaskDefinition>) -> Self {
        self.task_definitions = definitions;
        self
    }

    /// Build the unified DAG for specific tasks
    pub fn build_for_tasks(self, task_names: &[String]) -> Result<UnifiedTaskDAG> {
        let mut dag = UnifiedTaskDAG {
            task_configs: self.task_configs,
            task_nodes: self.task_nodes,
            task_definitions: self.task_definitions,
            execution_graph: Vec::new(),
            dependencies: HashMap::new(),
        };

        // Build the execution graph for the requested tasks
        dag.build_execution_graph(task_names)?;

        Ok(dag)
    }
}

impl Default for DAGBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedTaskDAG {
    /// Create a new DAG builder
    pub fn builder() -> DAGBuilder {
        DAGBuilder::new()
    }

    /// Build the execution graph for specific tasks
    fn build_execution_graph(&mut self, task_names: &[String]) -> Result<()> {
        let mut all_flattened_tasks = Vec::new();

        // Process each requested task or group
        for task_name in task_names {
            if let Some(task_config) = self.task_configs.get(task_name) {
                // It's a regular task - collect its dependencies
                self.collect_task_dependencies(task_name, task_config, &mut all_flattened_tasks)?;
            } else if let Some(task_node) = self.task_nodes.get(task_name) {
                // It's a task group - use strategy to flatten it
                self.collect_group_dependencies(task_name, task_node, &mut all_flattened_tasks)?;
            } else {
                return Err(cuenv_core::Error::configuration(format!(
                    "Task or task group '{task_name}' not found"
                )));
            }
        }

        self.execution_graph = all_flattened_tasks;
        self.build_dependency_map()?;
        self.build_task_definitions_for_flattened_tasks()?;

        Ok(())
    }

    /// Collect dependencies for a regular task
    fn collect_task_dependencies(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        flattened_tasks: &mut Vec<FlattenedTask>,
    ) -> Result<()> {
        // If this task has dependencies, collect them first
        if let Some(deps) = &task_config.dependencies {
            for dep_name in deps {
                if let Some(dep_node) = self.task_nodes.get(dep_name) {
                    // Dependency is a group - flatten it using appropriate strategy
                    self.collect_group_dependencies(dep_name, dep_node, flattened_tasks)?;
                } else if let Some(dep_config) = self.task_configs.get(dep_name) {
                    // Dependency is a regular task - recurse
                    self.collect_task_dependencies(dep_name, dep_config, flattened_tasks)?;
                } else {
                    return Err(cuenv_core::Error::configuration(format!(
                        "Dependency '{dep_name}' not found for task '{task_name}'"
                    )));
                }
            }
        }

        // Add this task if not already added
        if !flattened_tasks
            .iter()
            .any(|t| t.name == task_name && !t.is_barrier)
        {
            // Resolve dependencies: if dependency is a group, point to its end barrier
            let mut resolved_dependencies = Vec::new();
            if let Some(deps) = &task_config.dependencies {
                for dep_name in deps {
                    if self.task_nodes.contains_key(dep_name) {
                        // This dependency is a group - depend on its end barrier
                        resolved_dependencies.push(format!("{dep_name}:__end__"));
                    } else if self.task_configs.contains_key(dep_name) {
                        // This dependency is a regular task
                        resolved_dependencies.push(dep_name.clone());
                    } else {
                        return Err(cuenv_core::Error::configuration(format!(
                            "Dependency '{dep_name}' not found for task '{task_name}'"
                        )));
                    }
                }
            }

            flattened_tasks.push(FlattenedTask {
                id: task_name.to_string(),
                group_path: Vec::new(),
                name: task_name.to_string(),
                dependencies: resolved_dependencies,
                node: TaskNode::Task(Box::new(task_config.clone())),
                is_barrier: false,
            });
        }

        Ok(())
    }

    /// Collect dependencies for a task group using appropriate strategy
    fn collect_group_dependencies(
        &self,
        group_name: &str,
        group_node: &TaskNode,
        flattened_tasks: &mut Vec<FlattenedTask>,
    ) -> Result<()> {
        match group_node {
            TaskNode::Group { tasks, .. } => {
                // Use the appropriate strategy based on collection type
                let strategy: Box<dyn GroupExecutionStrategy> = match tasks {
                    TaskCollection::Sequential(_) => Box::new(SequentialStrategy),
                    TaskCollection::Parallel(_) => Box::new(GroupStrategy),
                };
                let group_flattened = strategy.process_group(group_name, tasks, Vec::new())?;

                // Add all tasks from this group
                for task in group_flattened {
                    // Avoid duplicates
                    if !flattened_tasks.iter().any(|t| t.id == task.id) {
                        flattened_tasks.push(task);
                    }
                }
            }
            TaskNode::Task(task_config) => {
                // Single task - collect its dependencies recursively
                self.collect_task_dependencies(group_name, task_config, flattened_tasks)?;
            }
        }

        Ok(())
    }

    /// Build the dependency map from flattened tasks
    fn build_dependency_map(&mut self) -> Result<()> {
        for task in &self.execution_graph {
            self.dependencies
                .insert(task.id.clone(), task.dependencies.clone());
        }
        Ok(())
    }

    /// Build task definitions for flattened tasks that don't have definitions yet
    fn build_task_definitions_for_flattened_tasks(&mut self) -> Result<()> {
        use cuenv_core::{TaskDefinition, TaskExecutionMode};
        use std::path::PathBuf;
        use std::time::Duration;

        for task in &self.execution_graph {
            // Skip barriers and tasks that already have definitions
            if task.is_barrier || self.task_definitions.contains_key(&task.id) {
                continue;
            }

            // Extract the underlying task config from the flattened task
            if let TaskNode::Task(task_config) = &task.node {
                let definition = TaskDefinition {
                    name: task.id.clone(),
                    description: task_config.description.clone(),
                    execution_mode: if let Some(command) = &task_config.command {
                        TaskExecutionMode::Command {
                            command: command.clone(),
                        }
                    } else if let Some(script) = &task_config.script {
                        TaskExecutionMode::Script {
                            content: script.clone(),
                        }
                    } else {
                        // Default to echo command for tasks without command/script
                        TaskExecutionMode::Command {
                            command: format!("echo 'Task {} executed'", task.name),
                        }
                    },
                    dependencies: task
                        .dependencies
                        .iter()
                        .map(|dep| cuenv_core::ResolvedDependency::new(dep.clone()))
                        .collect(),
                    working_directory: task_config
                        .working_dir
                        .as_ref()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from(".")),
                    shell: task_config
                        .shell
                        .clone()
                        .unwrap_or_else(|| "sh".to_string()),
                    inputs: task_config.inputs.clone().unwrap_or_default(),
                    outputs: task_config.outputs.clone().unwrap_or_default(),
                    security: None, // TODO: Convert from task_config.security
                    cache: cuenv_core::TaskCache::default(), // TODO: Convert from task_config.cache
                    timeout: Duration::from_secs(300), // TODO: Extract from config if available
                };

                self.task_definitions.insert(task.id.clone(), definition);
            }
        }

        Ok(())
    }

    /// Get the execution order using topological sort
    pub fn get_execution_levels(&self) -> Result<Vec<Vec<String>>> {
        super::graph::topological_sort(&self.dependencies)
    }

    /// Get all flattened tasks in the execution graph
    pub fn get_flattened_tasks(&self) -> &[FlattenedTask] {
        &self.execution_graph
    }

    /// Get task definitions for execution
    pub fn get_task_definitions(&self) -> &HashMap<String, TaskDefinition> {
        &self.task_definitions
    }

    /// Get a task definition by ID
    pub fn get_task_definition(&self, task_id: &str) -> Option<&TaskDefinition> {
        // For flattened task IDs that might include group paths, try various lookups
        if let Some(def) = self.task_definitions.get(task_id) {
            return Some(def);
        }

        // Try extracting just the task name from complex IDs
        if let Some(task_name) = task_id.split(':').next_back() {
            if let Some(def) = self.task_definitions.get(task_name) {
                return Some(def);
            }
        }

        None
    }

    /// Get dependencies for a specific task
    pub fn get_task_dependencies(&self, task_id: &str) -> Option<&[String]> {
        self.dependencies.get(task_id).map(|deps| deps.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::{TaskCollection, TaskConfig};

    fn create_test_config(command: &str, deps: Option<Vec<String>>) -> TaskConfig {
        TaskConfig {
            command: Some(command.to_string()),
            dependencies: deps,
            ..Default::default()
        }
    }

    fn create_test_group(tasks: TaskCollection) -> TaskNode {
        TaskNode::Group {
            tasks,
            description: None,
        }
    }

    #[test]
    fn test_dag_builder_basic() {
        let mut task_configs = HashMap::new();
        task_configs.insert("test".to_string(), create_test_config("echo test", None));

        let dag = UnifiedTaskDAG::builder()
            .with_task_configs(task_configs)
            .build_for_tasks(&["test".to_string()])
            .unwrap();

        assert_eq!(dag.execution_graph.len(), 1);
        assert_eq!(dag.execution_graph[0].name, "test");
    }

    #[test]
    fn test_dag_with_dependencies() {
        let mut task_configs = HashMap::new();
        task_configs.insert("task1".to_string(), create_test_config("echo 1", None));
        task_configs.insert(
            "task2".to_string(),
            create_test_config("echo 2", Some(vec!["task1".to_string()])),
        );

        let dag = UnifiedTaskDAG::builder()
            .with_task_configs(task_configs)
            .build_for_tasks(&["task2".to_string()])
            .unwrap();

        // Should have both tasks (task2 and its dependency task1)
        assert_eq!(dag.execution_graph.len(), 2);

        // Check that dependencies are properly set
        let levels = dag.get_execution_levels().unwrap();
        assert_eq!(levels.len(), 2); // Two execution levels
        assert_eq!(levels[0], vec!["task1".to_string()]); // task1 first
        assert_eq!(levels[1], vec!["task2".to_string()]); // task2 second
    }

    #[test]
    fn test_dag_with_sequential_group() {
        let task_configs = HashMap::new();
        let mut task_nodes = IndexMap::new();

        // Create a sequential group
        let mut group_tasks = IndexMap::new();
        group_tasks.insert(
            "one".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 1", None))),
        );
        group_tasks.insert(
            "two".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 2", None))),
        );

        task_nodes.insert(
            "count".to_string(),
            create_test_group(TaskCollection::Sequential(vec![
                TaskNode::Task(Box::new(create_test_config("echo 1", None))),
                TaskNode::Task(Box::new(create_test_config("echo 2", None))),
            ])),
        );

        let dag = UnifiedTaskDAG::builder()
            .with_task_configs(task_configs)
            .with_task_nodes(task_nodes)
            .build_for_tasks(&["count".to_string()])
            .unwrap();

        // Should have start barrier, two tasks, end barrier = 4 items
        assert_eq!(dag.execution_graph.len(), 4);

        // Verify sequential ordering in execution levels
        let levels = dag.get_execution_levels().unwrap();
        assert!(levels.len() >= 2); // At least 2 levels due to sequential dependencies
    }

    #[test]
    fn test_sequential_ordering_preservation() {
        let task_configs = HashMap::new();
        let mut task_nodes = IndexMap::new();

        // Create a sequential group with the exact same structure as env.cue
        let mut group_tasks = IndexMap::new();

        // Insert in the exact order defined in env.cue: one, two, three, four
        group_tasks.insert(
            "one".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 1", None))),
        );
        group_tasks.insert(
            "two".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 2", None))),
        );
        group_tasks.insert(
            "three".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 3", None))),
        );
        group_tasks.insert(
            "four".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 4", None))),
        );

        // Verify IndexMap preserves order
        let names: Vec<_> = group_tasks.keys().cloned().collect();
        assert_eq!(names, vec!["one", "two", "three", "four"]);

        task_nodes.insert(
            "count".to_string(),
            create_test_group(TaskCollection::Sequential(vec![
                TaskNode::Task(Box::new(create_test_config("echo 1", None))),
                TaskNode::Task(Box::new(create_test_config("echo 2", None))),
                TaskNode::Task(Box::new(create_test_config("echo 3", None))),
                TaskNode::Task(Box::new(create_test_config("echo 4", None))),
            ])),
        );

        let dag = UnifiedTaskDAG::builder()
            .with_task_configs(task_configs)
            .with_task_nodes(task_nodes)
            .build_for_tasks(&["count".to_string()])
            .unwrap();

        tracing::debug!("Flattened tasks from unified DAG:");
        for task in dag.get_flattened_tasks() {
            if !task.is_barrier {
                tracing::debug!("  Task: {} (ID: {})", task.name, task.id);
            }
        }

        tracing::debug!("Execution levels:");
        let levels = dag.get_execution_levels().unwrap();
        for (i, level) in levels.iter().enumerate() {
            tracing::debug!("  Level {i}: {level:?}");
        }

        // The execution should preserve the sequential order
        // Find the regular tasks (excluding barriers) in execution order
        let mut execution_order = Vec::new();
        for level in &levels {
            for task_id in level {
                if !task_id.contains("__") {
                    // Skip barriers that contain "__"
                    execution_order.push(task_id.clone());
                }
            }
        }

        tracing::debug!("Final execution order: {execution_order:?}");

        // Should be: count:task_0, count:task_1, count:task_2, count:task_3 in sequential order
        let expected_tasks = [
            "count:task_0",
            "count:task_1",
            "count:task_2",
            "count:task_3",
        ];
        let actual_task_names: Vec<String> = execution_order
            .into_iter()
            .filter(|task_id| expected_tasks.iter().any(|expected| task_id == expected))
            .collect();

        tracing::debug!("Filtered task names: {actual_task_names:?}");

        // Verify we have the expected sequential tasks with auto-generated names
        assert_eq!(
            actual_task_names,
            vec![
                "count:task_0".to_string(),
                "count:task_1".to_string(),
                "count:task_2".to_string(),
                "count:task_3".to_string()
            ],
            "Tasks should be in sequential order with auto-generated names"
        );
    }

    #[test]
    fn test_task_depending_on_group() {
        let mut task_configs = HashMap::new();
        let mut task_nodes = IndexMap::new();

        // Create the counted task that depends on count group
        task_configs.insert(
            "counted".to_string(),
            create_test_config("echo counted", Some(vec!["count".to_string()])),
        );

        // Create a sequential group
        let mut group_tasks = IndexMap::new();
        group_tasks.insert(
            "one".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 1", None))),
        );
        group_tasks.insert(
            "two".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 2", None))),
        );
        group_tasks.insert(
            "three".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 3", None))),
        );
        group_tasks.insert(
            "four".to_string(),
            TaskNode::Task(Box::new(create_test_config("echo 4", None))),
        );

        task_nodes.insert(
            "count".to_string(),
            create_test_group(TaskCollection::Sequential(vec![
                TaskNode::Task(Box::new(create_test_config("echo 1", None))),
                TaskNode::Task(Box::new(create_test_config("echo 2", None))),
                TaskNode::Task(Box::new(create_test_config("echo 3", None))),
                TaskNode::Task(Box::new(create_test_config("echo 4", None))),
            ])),
        );

        let dag = UnifiedTaskDAG::builder()
            .with_task_configs(task_configs)
            .with_task_nodes(task_nodes)
            .build_for_tasks(&["counted".to_string()])
            .unwrap();

        tracing::debug!("Task->Group dependency - Flattened tasks:");
        for task in dag.get_flattened_tasks() {
            if !task.is_barrier {
                tracing::debug!("  Task: {} (ID: {})", task.name, task.id);
            } else {
                tracing::debug!("  Barrier: {} (ID: {})", task.name, task.id);
            }
        }

        tracing::debug!("Task->Group dependency - Execution levels:");
        let levels = dag.get_execution_levels().unwrap();
        for (i, level) in levels.iter().enumerate() {
            tracing::debug!("  Level {i}: {level:?}");
        }

        // Should have the count group tasks plus the counted task
        let regular_tasks: Vec<_> = dag
            .get_flattened_tasks()
            .iter()
            .filter(|t| !t.is_barrier)
            .collect();
        assert!(
            regular_tasks.len() >= 5,
            "Should have at least 5 tasks (4 from group + 1 counted), got: {}",
            regular_tasks.len()
        );
    }
}
