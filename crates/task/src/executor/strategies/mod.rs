//! Task group execution strategies

use cuenv_config::{TaskCollection, TaskNode};
use cuenv_core::Result;

mod group;
mod integration;
mod parallel;
mod sequential;

#[cfg(test)]
mod tests;

pub use group::GroupStrategy;
pub use integration::{process_task_group, TaskGroupExecutionPlan};
pub use sequential::SequentialStrategy;

/// Trait for task group execution strategies
pub trait GroupExecutionStrategy {
    /// Process a task group and return flattened tasks with dependencies
    fn process_group(
        &self,
        group_name: &str,
        tasks: &TaskCollection,
        parent_path: Vec<String>,
    ) -> Result<Vec<FlattenedTask>>;
}

/// A flattened task with resolved dependencies
#[derive(Debug, Clone)]
pub struct FlattenedTask {
    /// Full task identifier (e.g., "ci.prepare:clean")  
    pub id: String,
    /// Path components (e.g., ["ci", "prepare"])
    pub group_path: Vec<String>,
    /// Task name within the group
    pub name: String,
    /// Dependencies (fully qualified IDs)
    pub dependencies: Vec<String>,
    /// The actual task node
    pub node: TaskNode,
    /// Whether this is a barrier task
    pub is_barrier: bool,
}

/// Helper function to create a barrier task
pub fn create_barrier_task(
    id: String,
    group_path: Vec<String>,
    dependencies: Vec<String>,
) -> FlattenedTask {
    FlattenedTask {
        id,
        group_path,
        name: "__barrier__".to_string(),
        dependencies,
        node: TaskNode::Task(Box::default()),
        is_barrier: true,
    }
}

/// Helper function to create a task ID from path components
pub fn create_task_id(path: &[String], name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{}:{}", path.join("."), name)
    }
}
