//! Group execution strategy (simple collection)

use super::{create_task_id, FlattenedTask, GroupExecutionStrategy};
use cuenv_config::TaskNode;
use cuenv_core::Result;
use indexmap::IndexMap;

/// Group execution strategy - simple collection with no special behavior
pub struct GroupStrategy;

impl GroupExecutionStrategy for GroupStrategy {
    fn process_group(
        &self,
        group_name: &str,
        tasks: &IndexMap<String, TaskNode>,
        parent_path: Vec<String>,
    ) -> Result<Vec<FlattenedTask>> {
        let mut flattened = Vec::new();
        let mut group_path = parent_path.clone();
        group_path.push(group_name.to_string());

        // Process each task independently
        for (task_name, node) in tasks {
            match node {
                TaskNode::Task(config) => {
                    // Resolve dependencies
                    let deps = config
                        .dependencies
                        .as_ref()
                        .map(|d| {
                            d.iter()
                                .map(|dep| {
                                    // Check if dependency is in same group or external
                                    if tasks.contains_key(dep) {
                                        // Internal dependency
                                        create_task_id(&group_path, dep)
                                    } else if dep.contains(':') {
                                        // Already qualified
                                        dep.clone()
                                    } else {
                                        // External dependency
                                        dep.clone()
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    flattened.push(FlattenedTask {
                        id: create_task_id(&group_path, task_name),
                        group_path: group_path.clone(),
                        name: task_name.clone(),
                        dependencies: deps,
                        node: node.clone(),
                        is_barrier: false,
                    });
                }
                TaskNode::Group {
                    mode,
                    tasks: subtasks,
                    ..
                } => {
                    // Recursively process subgroup with its own strategy
                    let strategy = super::create_strategy(mode);
                    let subtask_path = group_path.clone();
                    let subgroup_tasks =
                        strategy.process_group(task_name, subtasks, subtask_path)?;
                    flattened.extend(subgroup_tasks);
                }
            }
        }

        Ok(flattened)
    }
}
