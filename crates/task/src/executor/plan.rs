use cuenv_core::TaskDefinition;
use std::collections::HashMap;

/// Represents a task execution plan with resolved dependencies
#[derive(Debug, Clone)]
pub struct TaskExecutionPlan {
    /// Tasks organized by execution level (level 0 = no dependencies, etc.)
    pub levels: Vec<Vec<String>>,
    /// Built and validated task definitions
    pub tasks: HashMap<String, TaskDefinition>,
}
