//! Parallel execution strategy

use super::{create_barrier_task, create_task_id, FlattenedTask, GroupExecutionStrategy};
use cuenv_config::{TaskCollection, TaskNode};
use cuenv_core::Result;

/// Parallel execution strategy - all tasks run simultaneously
#[allow(dead_code)]
pub struct ParallelStrategy;

impl GroupExecutionStrategy for ParallelStrategy {
    fn process_group(
        &self,
        group_name: &str,
        tasks: &TaskCollection,
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

        let mut task_ids = Vec::new();

        // Process all tasks - they all depend only on the start barrier
        for (task_name, node) in tasks.iter() {
            match node {
                TaskNode::Task(config) => {
                    // Start with dependency on start barrier
                    let mut deps = vec![start_barrier_id.clone()];

                    // Add any explicit external dependencies
                    if let Some(explicit_deps) = &config.dependencies {
                        for dep in explicit_deps {
                            // Check if this is an internal dependency
                            let has_internal_dep = match tasks {
                                TaskCollection::Sequential(_) => {
                                    // For sequential collections, internal dependencies don't make sense
                                    false
                                }
                                TaskCollection::Parallel(task_map) => task_map.contains_key(dep),
                            };

                            if has_internal_dep {
                                // Internal dependency - ignore in parallel mode
                                // All tasks in parallel group run simultaneously
                                continue;
                            } else {
                                // External dependency
                                deps.push(dep.clone());
                            }
                        }
                    }

                    let task_id = create_task_id(&group_path, &task_name);
                    task_ids.push(task_id.clone());

                    flattened.push(FlattenedTask {
                        id: task_id,
                        group_path: group_path.clone(),
                        name: task_name.clone(),
                        dependencies: deps,
                        node: node.clone(),
                        is_barrier: false,
                    });
                }
                TaskNode::Group {
                    tasks: subtasks, ..
                } => {
                    // Create a subgroup that depends on start barrier
                    let subgroup_start =
                        create_task_id(&group_path, &format!("{task_name}:__start__"));
                    flattened.push(create_barrier_task(
                        subgroup_start.clone(),
                        group_path.clone(),
                        vec![start_barrier_id.clone()],
                    ));

                    // Process subgroup using appropriate strategy
                    let strategy: Box<dyn GroupExecutionStrategy> = match subtasks {
                        TaskCollection::Sequential(_) => Box::new(super::SequentialStrategy),
                        TaskCollection::Parallel(_) => Box::new(ParallelStrategy),
                    };
                    let subtask_path = group_path.clone();
                    let mut subgroup_tasks =
                        strategy.process_group(&task_name, subtasks, subtask_path)?;

                    // Update first tasks in subgroup to depend on subgroup start
                    for task in &mut subgroup_tasks {
                        if !task.is_barrier && task.dependencies.is_empty() {
                            task.dependencies.push(subgroup_start.clone());
                        }
                    }

                    // Collect all task IDs from subgroup for end barrier
                    let subgroup_task_ids: Vec<String> = subgroup_tasks
                        .iter()
                        .filter(|t| !t.is_barrier)
                        .map(|t| t.id.clone())
                        .collect();

                    flattened.extend(subgroup_tasks);

                    // Create subgroup end barrier
                    let subgroup_end = create_task_id(&group_path, &format!("{task_name}:__end__"));
                    flattened.push(create_barrier_task(
                        subgroup_end.clone(),
                        group_path.clone(),
                        subgroup_task_ids,
                    ));

                    task_ids.push(subgroup_end);
                }
            }
        }

        // Create end barrier that depends on all tasks
        let end_barrier_id = create_task_id(&group_path, "__end__");
        flattened.push(create_barrier_task(
            end_barrier_id,
            group_path.clone(),
            task_ids,
        ));

        Ok(flattened)
    }
}
