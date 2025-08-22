//! Petgraph-based Task DAG implementation
//!
//! This module provides a proper DAG implementation using petgraph for correct
//! dependency management, topological sorting, and cycle detection.

use cuenv_config::{TaskConfig, TaskNode};
use cuenv_core::{Error, Result, TaskDefinition};
use indexmap::IndexMap;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::{algo, Direction};
use std::collections::HashMap;

use super::strategies::FlattenedTask;

/// Task node data stored in the graph
#[derive(Debug, Clone)]
pub struct TaskNodeData {
    pub id: String,
    pub name: String,
    pub group_path: Vec<String>,
    pub node: TaskNode,
    pub is_barrier: bool,
}

/// Edge type for dependencies
#[derive(Debug, Clone, PartialEq)]
pub enum DependencyType {
    /// Task depends on another task
    TaskDependency,
}

/// Petgraph-based Task DAG
#[derive(Debug, Clone)]
pub struct TaskDAG {
    /// The directed graph
    graph: DiGraph<TaskNodeData, DependencyType>,
    /// Map from task ID to graph node index
    task_map: HashMap<String, NodeIndex>,
    /// Task definitions built from configs
    task_definitions: HashMap<String, TaskDefinition>,
    /// Root task being executed
    root_task: String,
}

impl TaskDAG {
    /// Create a new DAG builder
    pub fn builder() -> TaskDAGBuilder {
        TaskDAGBuilder::new()
    }

    /// Get all flattened tasks in topological order
    pub fn get_flattened_tasks(&self) -> Vec<FlattenedTask> {
        match algo::toposort(&self.graph, None) {
            Ok(sorted_nodes) => sorted_nodes
                .into_iter()
                .map(|node_idx| {
                    let task_data = &self.graph[node_idx];

                    // Get dependencies for this task
                    let dependencies: Vec<String> = self
                        .graph
                        .edges_directed(node_idx, Direction::Outgoing)
                        .map(|edge| self.graph[edge.target()].id.clone())
                        .collect();

                    FlattenedTask {
                        id: task_data.id.clone(),
                        group_path: task_data.group_path.clone(),
                        name: task_data.name.clone(),
                        dependencies,
                        node: task_data.node.clone(),
                        is_barrier: task_data.is_barrier,
                    }
                })
                .collect(),
            Err(_cycle_node) => {
                // Handle cycle - for now return empty, but we should handle this better
                Vec::new()
            }
        }
    }

    /// Get execution levels (tasks that can run in parallel)
    pub fn get_execution_levels(&self) -> Result<Vec<Vec<String>>> {
        // Use topological sorting to get execution order
        let sorted_nodes = algo::toposort(&self.graph, None)
            .map_err(|_| Error::configuration("Circular dependency detected in task graph"))?;

        // Build levels based on dependencies
        let mut levels = Vec::new();
        let mut remaining: std::collections::HashSet<NodeIndex> =
            sorted_nodes.iter().cloned().collect();

        while !remaining.is_empty() {
            let mut current_level = Vec::new();
            let mut to_remove = Vec::new();

            // Find all nodes that have no remaining dependencies
            for &node_idx in &remaining {
                let has_unresolved_deps = self
                    .graph
                    .edges_directed(node_idx, Direction::Outgoing)
                    .any(|edge| remaining.contains(&edge.target()));

                if !has_unresolved_deps {
                    current_level.push(self.graph[node_idx].id.clone());
                    to_remove.push(node_idx);
                }
            }

            // Remove processed nodes
            for node_idx in to_remove {
                remaining.remove(&node_idx);
            }

            if !current_level.is_empty() {
                levels.push(current_level);
            } else {
                // Prevent infinite loop - this shouldn't happen with acyclic graphs
                break;
            }
        }

        Ok(levels)
    }

    /// Get dependencies for a specific task
    pub fn get_task_dependencies(&self, task_name: &str) -> Option<Vec<String>> {
        let node_idx = self.task_map.get(task_name)?;
        Some(
            self.graph
                .edges_directed(*node_idx, Direction::Outgoing)
                .map(|edge| self.graph[edge.target()].id.clone())
                .collect(),
        )
    }

    /// Get task dependents (tasks that depend on this task)
    pub fn get_task_dependents(&self, task_name: &str) -> Option<Vec<String>> {
        let node_idx = self.task_map.get(task_name)?;
        Some(
            self.graph
                .edges_directed(*node_idx, Direction::Incoming)
                .map(|edge| self.graph[edge.source()].id.clone())
                .collect(),
        )
    }

    /// Get root nodes (tasks with no dependents)
    pub fn get_root_nodes(&self) -> Vec<String> {
        self.graph
            .node_indices()
            .filter(|&node_idx| {
                // Root nodes have no incoming edges (no tasks depend on them)
                self.graph
                    .edges_directed(node_idx, Direction::Incoming)
                    .count()
                    == 0
            })
            .map(|node_idx| self.graph[node_idx].id.clone())
            .collect()
    }

    /// Check if the graph has cycles
    pub fn is_cyclic(&self) -> bool {
        algo::is_cyclic_directed(&self.graph)
    }

    /// Get task definitions
    pub fn get_task_definitions(&self) -> &HashMap<String, TaskDefinition> {
        &self.task_definitions
    }

    /// Get a specific task definition by ID
    pub fn get_task_definition(&self, task_id: &str) -> Option<&TaskDefinition> {
        self.task_definitions.get(task_id)
    }

    /// Get the root task name
    pub fn get_root_task(&self) -> &str {
        &self.root_task
    }
}

/// Builder for TaskDAG
pub struct TaskDAGBuilder {
    task_configs: HashMap<String, TaskConfig>,
    task_nodes: IndexMap<String, TaskNode>,
    task_definitions: HashMap<String, TaskDefinition>,
}

impl TaskDAGBuilder {
    pub fn new() -> Self {
        Self {
            task_configs: HashMap::new(),
            task_nodes: IndexMap::new(),
            task_definitions: HashMap::new(),
        }
    }

    pub fn with_task_configs(mut self, configs: HashMap<String, TaskConfig>) -> Self {
        self.task_configs = configs;
        self
    }

    pub fn with_task_nodes(mut self, nodes: IndexMap<String, TaskNode>) -> Self {
        self.task_nodes = nodes;
        self
    }

    pub fn with_task_definitions(mut self, definitions: HashMap<String, TaskDefinition>) -> Self {
        self.task_definitions = definitions;
        self
    }

    /// Build the DAG for specified tasks
    pub fn build_for_tasks(self, task_names: &[String]) -> Result<TaskDAG> {
        if task_names.is_empty() {
            return Err(Error::configuration("No tasks specified"));
        }

        let root_task = task_names[0].clone();
        let mut graph = DiGraph::new();
        let mut task_map = HashMap::new();

        // Collect all tasks that need to be included (requested tasks + their transitive dependencies)
        let mut tasks_to_include = std::collections::HashSet::new();
        let mut to_process = std::collections::VecDeque::from_iter(task_names.iter().cloned());

        while let Some(task_id) = to_process.pop_front() {
            if !tasks_to_include.contains(&task_id) {
                tasks_to_include.insert(task_id.clone());

                // Add dependencies to processing queue
                if let Some(task_def) = self.task_definitions.get(&task_id) {
                    for dep in &task_def.dependencies {
                        if !tasks_to_include.contains(&dep.name) {
                            to_process.push_back(dep.name.clone());
                        }
                    }
                }
            }
        }

        // Add only the relevant task nodes to the graph
        for task_id in &tasks_to_include {
            if let Some(_task_def) = self.task_definitions.get(task_id) {
                let task_data = TaskNodeData {
                    id: task_id.clone(),
                    name: task_id.clone(),  // TODO: extract actual name
                    group_path: Vec::new(), // TODO: extract group path
                    node: TaskNode::Task(Box::default()), // TODO: get actual node
                    is_barrier: task_id.contains("__"),
                };

                let node_idx = graph.add_node(task_data);
                task_map.insert(task_id.clone(), node_idx);
            }
        }

        // Then, add edges for dependencies (FROM task TO dependency)
        for task_id in &tasks_to_include {
            if let Some(&task_node_idx) = task_map.get(task_id) {
                if let Some(_task_def) = self.task_definitions.get(task_id) {
                    for dep in &_task_def.dependencies {
                        if let Some(&dep_node_idx) = task_map.get(&dep.name) {
                            // Add edge FROM task TO dependency (correct direction!)
                            graph.add_edge(
                                task_node_idx,
                                dep_node_idx,
                                DependencyType::TaskDependency,
                            );
                        }
                    }
                }
            }
        }

        // Filter task_definitions to only include tasks that are in the graph
        let filtered_task_definitions: HashMap<String, TaskDefinition> = self
            .task_definitions
            .into_iter()
            .filter(|(task_id, _)| tasks_to_include.contains(task_id))
            .collect();

        let dag = TaskDAG {
            graph,
            task_map,
            task_definitions: filtered_task_definitions,
            root_task,
        };

        // Check for cycles
        if dag.is_cyclic() {
            return Err(Error::configuration(
                "Circular dependency detected in task graph",
            ));
        }

        Ok(dag)
    }
}
