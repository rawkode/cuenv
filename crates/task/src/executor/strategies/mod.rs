//! Task group execution strategies

use cuenv_config::{TaskGroupMode, TaskNode};
use cuenv_core::Result;
use std::collections::HashMap;

mod group;
mod integration;
mod parallel;
mod sequential;
mod workflow;

#[cfg(test)]
mod tests;

pub use group::GroupStrategy;
pub use integration::{process_task_group, TaskGroupExecutionPlan};
pub use parallel::ParallelStrategy;
pub use sequential::SequentialStrategy;
pub use workflow::WorkflowStrategy;

/// Trait for task group execution strategies
pub trait GroupExecutionStrategy {
    /// Process a task group and return flattened tasks with dependencies
    fn process_group(
        &self,
        group_name: &str,
        tasks: &HashMap<String, TaskNode>,
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

use std::sync::Arc;

/// Get a cached strategy based on the group mode
/// 
/// Strategies are stateless and can be safely shared across threads.
/// This function returns Arc-wrapped instances to avoid repeated allocations.
pub fn get_cached_strategy(mode: &TaskGroupMode) -> Arc<dyn GroupExecutionStrategy> {
    match mode {
        TaskGroupMode::Workflow => Arc::new(WorkflowStrategy),
        TaskGroupMode::Sequential => Arc::new(SequentialStrategy),
        TaskGroupMode::Parallel => Arc::new(ParallelStrategy),
        TaskGroupMode::Group => Arc::new(GroupStrategy),
    }
}

/// Create a strategy based on the group mode (for backward compatibility)
#[inline]
pub fn create_strategy(mode: &TaskGroupMode) -> Box<dyn GroupExecutionStrategy> {
    match mode {
        TaskGroupMode::Workflow => Box::new(WorkflowStrategy),
        TaskGroupMode::Sequential => Box::new(SequentialStrategy),
        TaskGroupMode::Parallel => Box::new(ParallelStrategy),
        TaskGroupMode::Group => Box::new(GroupStrategy),
    }
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
