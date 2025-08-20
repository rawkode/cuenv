//! Sequential execution strategy

use super::{create_barrier_task, create_task_id, FlattenedTask, GroupExecutionStrategy};
use cuenv_config::{TaskCollection, TaskNode};
use cuenv_core::Result;

/// Sequential execution strategy - tasks run one after another
pub struct SequentialStrategy;

impl GroupExecutionStrategy for SequentialStrategy {
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

        let mut prev_task_id = start_barrier_id;

        // Process tasks in order - arrays naturally preserve order!
        for (task_name, node) in tasks.iter() {
            match &node {
                TaskNode::Task(config) => {
                    // In sequential mode, each task depends on the previous one
                    // Plus any explicit dependencies
                    let mut deps = vec![prev_task_id.clone()];

                    if let Some(explicit_deps) = &config.dependencies {
                        for dep in explicit_deps {
                            // In sequential mode, explicit dependencies should be external
                            // Internal dependencies are handled by sequential order
                            deps.push(dep.clone());
                        }
                    }

                    let task_id = create_task_id(&group_path, &task_name);
                    flattened.push(FlattenedTask {
                        id: task_id.clone(),
                        group_path: group_path.clone(),
                        name: task_name.clone(),
                        dependencies: deps,
                        node: node.clone(),
                        is_barrier: false,
                    });

                    prev_task_id = task_id;
                }
                TaskNode::Group {
                    tasks: subtasks, ..
                } => {
                    // Create a subgroup with sequential strategy for nested groups
                    let subgroup_start =
                        create_task_id(&group_path, &format!("{task_name}:__start__"));
                    flattened.push(create_barrier_task(
                        subgroup_start.clone(),
                        group_path.clone(),
                        vec![prev_task_id.clone()],
                    ));

                    // Process subgroup using appropriate strategy based on collection type
                    let strategy: Box<dyn GroupExecutionStrategy> = match subtasks {
                        TaskCollection::Sequential(_) => Box::new(SequentialStrategy),
                        TaskCollection::Parallel(_) => Box::new(super::GroupStrategy),
                    };
                    let subtask_path = group_path.clone();
                    let mut subgroup_tasks =
                        strategy.process_group(&task_name, subtasks, subtask_path)?;

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
