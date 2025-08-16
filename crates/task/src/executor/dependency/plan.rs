use crate::executor::graph;
use crate::executor::plan::TaskExecutionPlan;
use crate::executor::TaskExecutor;
use crate::MonorepoTaskRegistry;
use cuenv_core::{Error, Result};
use std::collections::{HashMap, HashSet};

impl TaskExecutor {
    /// Build an execution plan with dependency resolution
    pub fn build_execution_plan(&self, task_names: &[String]) -> Result<TaskExecutionPlan> {
        // If we have a monorepo registry, use it for cross-package task resolution
        if let Some(ref registry) = self.monorepo_registry {
            return self.build_monorepo_execution_plan(task_names, registry);
        }

        let all_task_configs = self.env_manager.get_tasks();
        let all_task_nodes = self.env_manager.get_task_nodes();

        // Validate that all requested tasks exist (could be tasks or task groups)
        for task_name in task_names {
            if !all_task_configs.contains_key(task_name) && !all_task_nodes.contains_key(task_name)
            {
                return Err(Error::configuration(format!(
                    "Task or task group '{task_name}' not found"
                )));
            }
        }

        // Build task definitions using TaskBuilder with task nodes
        let task_definitions = self
            .task_builder
            .build_tasks_with_nodes(all_task_configs.clone(), all_task_nodes.clone())?;

        // Build dependency graph using task definitions
        let mut task_dependencies = HashMap::with_capacity(task_definitions.len());
        let mut visited = HashSet::with_capacity(task_definitions.len());
        let mut stack = HashSet::new();

        for task_name in task_names {
            super::collector::collect_dependencies_from_definitions(
                task_name,
                &task_definitions,
                &mut task_dependencies,
                &mut visited,
                &mut stack,
            )?;
        }

        // Topological sort to determine execution order
        let levels = graph::topological_sort(&task_dependencies)?;

        // Build final execution plan
        let mut plan_tasks = HashMap::with_capacity(task_dependencies.len());
        for task_name in task_dependencies.keys() {
            if let Some(definition) = task_definitions.get(task_name) {
                plan_tasks.insert(task_name.clone(), definition.clone());
            }
        }

        Ok(TaskExecutionPlan {
            levels,
            tasks: plan_tasks,
        })
    }

    /// Build an execution plan for monorepo with cross-package tasks
    pub(crate) fn build_monorepo_execution_plan(
        &self,
        task_names: &[String],
        registry: &MonorepoTaskRegistry,
    ) -> Result<TaskExecutionPlan> {
        let mut all_tasks = HashMap::new();
        let mut task_dependencies = HashMap::new();
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();

        // Validate and collect tasks from registry
        for task_name in task_names {
            let _task = registry
                .get_task(task_name)
                .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

            super::monorepo::collect_monorepo_dependencies(
                task_name,
                registry,
                &mut all_tasks,
                &mut task_dependencies,
                &mut visited,
                &mut stack,
            )?;
        }

        // Build task definitions using TaskBuilder
        let task_definitions = self.task_builder.build_tasks(all_tasks)?;

        // Topological sort to determine execution order
        let levels = graph::topological_sort(&task_dependencies)?;

        Ok(TaskExecutionPlan {
            levels,
            tasks: task_definitions,
        })
    }
}
