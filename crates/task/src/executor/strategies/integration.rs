//! Integration module for task group execution strategies
//!
//! This module integrates the execution strategies with the TaskExecutor,
//! providing methods to process task groups and their flattened representations.

use super::{GroupExecutionStrategy, GroupStrategy, SequentialStrategy};
use cuenv_config::{TaskCollection, TaskNode};
use cuenv_core::Result;
use std::collections::HashMap;

/// Process a task group and return the execution plan
pub fn process_task_group(
    group_name: &str,
    tasks: &TaskCollection,
) -> Result<TaskGroupExecutionPlan> {
    let flattened_tasks = match tasks {
        TaskCollection::Sequential(_) => {
            let strategy = SequentialStrategy;
            strategy.process_group(group_name, tasks, vec![])?
        }
        TaskCollection::Parallel(_) => {
            let strategy = GroupStrategy;
            strategy.process_group(group_name, tasks, vec![])?
        }
    };

    // Build the execution plan from flattened tasks
    let mut plan = TaskGroupExecutionPlan {
        group_name: group_name.to_string(),
        collection_type: tasks.clone(),
        tasks: Vec::new(),
        dependencies: HashMap::new(),
    };

    for task in flattened_tasks {
        // Use all fields of FlattenedTask to build the plan
        let task_info = ProcessedTaskInfo {
            id: task.id.clone(),
            name: task.name.clone(),
            group_path: task.group_path.clone(),
            is_barrier: task.is_barrier,
            node: task.node.clone(),
        };

        // Store dependencies for topological sorting
        plan.dependencies.insert(task.id.clone(), task.dependencies);
        plan.tasks.push(task_info);
    }

    Ok(plan)
}

/// Represents a processed task group ready for execution
#[derive(Debug, Clone)]
pub struct TaskGroupExecutionPlan {
    /// Name of the task group
    pub group_name: String,
    /// Task collection type (Sequential or Parallel)
    pub collection_type: TaskCollection,
    /// Processed tasks with all metadata
    pub tasks: Vec<ProcessedTaskInfo>,
    /// Task dependencies for execution ordering
    pub dependencies: HashMap<String, Vec<String>>,
}

/// Information about a processed task
#[derive(Debug, Clone)]
pub struct ProcessedTaskInfo {
    /// Full task identifier
    pub id: String,
    /// Task name within the group
    pub name: String,
    /// Path components leading to this task
    pub group_path: Vec<String>,
    /// Whether this is a barrier task for synchronization
    pub is_barrier: bool,
    /// The actual task node
    pub node: TaskNode,
}

impl ProcessedTaskInfo {
    /// Get the full path as a string
    pub fn full_path(&self) -> String {
        self.group_path.join(".")
    }

    /// Check if this task is executable (not a barrier)
    pub fn is_executable(&self) -> bool {
        !self.is_barrier && matches!(self.node, TaskNode::Task(_))
    }

    /// Get the task definition if this is an executable task
    pub fn get_task_definition(&self) -> Option<&cuenv_config::TaskConfig> {
        match &self.node {
            TaskNode::Task(config) => Some(config),
            _ => None,
        }
    }
}

impl TaskGroupExecutionPlan {
    /// Get all executable tasks (non-barriers)
    pub fn executable_tasks(&self) -> Vec<&ProcessedTaskInfo> {
        self.tasks.iter().filter(|t| t.is_executable()).collect()
    }

    /// Get all barrier tasks
    pub fn barrier_tasks(&self) -> Vec<&ProcessedTaskInfo> {
        self.tasks.iter().filter(|t| t.is_barrier).collect()
    }

    /// Build a topologically sorted execution order
    pub fn build_execution_order(&self) -> Result<Vec<String>> {
        use crate::executor::graph::topological_sort;

        let levels = topological_sort(&self.dependencies)?;
        let mut order = Vec::new();

        for level in levels {
            for task_id in level {
                // Only include executable tasks in the final order
                if let Some(task) = self.tasks.iter().find(|t| t.id == task_id) {
                    if task.is_executable() {
                        order.push(task_id);
                    }
                }
            }
        }

        Ok(order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::TaskConfig;
    use indexmap::IndexMap;

    #[test]
    fn test_process_task_group() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "build".to_string(),
            TaskNode::Task(Box::new(TaskConfig {
                command: Some("echo build".to_string()),
                description: Some("Build task".to_string()),
                ..Default::default()
            })),
        );
        tasks.insert(
            "test".to_string(),
            TaskNode::Task(Box::new(TaskConfig {
                command: Some("echo test".to_string()),
                dependencies: Some(vec!["build".to_string()]),
                ..Default::default()
            })),
        );

        let collection = TaskCollection::Parallel(tasks);
        let plan = process_task_group("ci", &collection).unwrap();

        assert_eq!(plan.group_name, "ci");
        assert!(matches!(plan.collection_type, TaskCollection::Parallel(_)));

        // Check that all tasks are processed
        let executable = plan.executable_tasks();
        assert_eq!(executable.len(), 2);

        // Verify task information is properly captured
        for task in &plan.tasks {
            if !task.is_barrier {
                assert!(!task.group_path.is_empty());
                assert!(!task.name.is_empty());
                assert!(!task.id.is_empty());
            }
        }
    }

    #[test]
    fn test_execution_plan_methods() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "task1".to_string(),
            TaskNode::Task(Box::new(TaskConfig {
                command: Some("echo 1".to_string()),
                ..Default::default()
            })),
        );

        let collection = TaskCollection::Parallel(tasks);
        let plan = process_task_group("test", &collection).unwrap();

        // Test executable_tasks method
        let executable = plan.executable_tasks();
        assert!(executable.iter().all(|t| t.is_executable()));

        // Test barrier_tasks method
        let barriers = plan.barrier_tasks();
        assert!(barriers.iter().all(|t| t.is_barrier));

        // Test that barriers and executable tasks are disjoint
        for task in &plan.tasks {
            assert!(task.is_barrier != task.is_executable());
        }
    }
}
