//! Sequential execution strategy

use super::{create_barrier_task, create_task_id, FlattenedTask, GroupExecutionStrategy};
use cuenv_config::TaskNode;
use cuenv_core::Result;
use std::collections::HashMap;

/// Sequential execution strategy - tasks run one after another
pub struct SequentialStrategy;

impl GroupExecutionStrategy for SequentialStrategy {
    fn process_group(
        &self,
        group_name: &str,
        tasks: &HashMap<String, TaskNode>,
        parent_path: Vec<String>,
    ) -> Result<Vec<FlattenedTask>> {
        let mut flattened = Vec::new();
        let mut group_path = parent_path.clone();
        group_path.push(group_name.to_string());

        // Create start barrier
        let start_barrier_id = create_task_id(&group_path, "__start__");
        flattened.push(create_barrier_task(
            start_barrier_id.clone(),
            group_path.clone(),
            vec![],
        ));

        let mut prev_task_id = start_barrier_id;

        // Process tasks in order (HashMap iteration order is not guaranteed,
        // but CUE preserves definition order in the JSON output)
        for (task_name, node) in tasks {
            match node {
                TaskNode::Task(config) => {
                    // In sequential mode, each task depends on the previous one
                    // Plus any explicit dependencies
                    let mut deps = vec![prev_task_id.clone()];

                    if let Some(explicit_deps) = &config.dependencies {
                        for dep in explicit_deps {
                            if tasks.contains_key(dep) {
                                // Internal dependency - this is an error in sequential mode
                                // as order is already determined
                                continue;
                            } else {
                                // External dependency
                                deps.push(dep.clone());
                            }
                        }
                    }

                    let task_id = create_task_id(&group_path, task_name);
                    flattened.push(FlattenedTask {
                        id: task_id.clone(),
                        group_path: group_path.clone(),
                        name: task_name.to_string(),
                        dependencies: deps,
                        node: node.clone(),
                        is_barrier: false,
                    });

                    prev_task_id = task_id;
                }
                TaskNode::Group {
                    mode,
                    tasks: subtasks,
                    ..
                } => {
                    // Create a subgroup with its own strategy
                    let subgroup_start =
                        create_task_id(&group_path, &format!("{task_name}:__start__"));
                    flattened.push(create_barrier_task(
                        subgroup_start.clone(),
                        group_path.clone(),
                        vec![prev_task_id.clone()],
                    ));

                    // Process subgroup
                    let strategy = super::create_strategy(mode);
                    let subtask_path = group_path.clone();
                    let mut subgroup_tasks =
                        strategy.process_group(task_name, subtasks, subtask_path)?;

                    // Update first task in subgroup to depend on subgroup start
                    if let Some(first) = subgroup_tasks.first_mut() {
                        first.dependencies.push(subgroup_start);
                    }

                    // Find the last task ID from the subgroup
                    let subgroup_end = create_task_id(&group_path, &format!("{task_name}:__end__"));
                    let last_subtask_ids: Vec<String> = subgroup_tasks
                        .iter()
                        .filter(|t| !t.is_barrier)
                        .map(|t| t.id.clone())
                        .collect();

                    flattened.extend(subgroup_tasks);
                    flattened.push(create_barrier_task(
                        subgroup_end.clone(),
                        group_path.clone(),
                        last_subtask_ids,
                    ));

                    prev_task_id = subgroup_end;
                }
            }
        }

        // Create end barrier
        let end_barrier_id = create_task_id(&group_path, "__end__");
        flattened.push(create_barrier_task(
            end_barrier_id,
            group_path.clone(),
            vec![prev_task_id],
        ));

        Ok(flattened)
    }
}
