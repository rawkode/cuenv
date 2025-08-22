use crate::executor::TaskExecutor;
use cuenv_core::{Error, Result};
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;

impl TaskExecutor {
    /// Internal method that supports output capture for TUI mode
    pub async fn execute_tasks_with_dependencies_internal(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
        capture_output: bool,
    ) -> Result<i32> {
        // Build execution plan
        let plan = self.build_execution_plan(task_names)?;

        // Create pipeline span for the entire execution
        // TODO: Add tracing when moved to workspace
        let _pipeline_span = tracing::info_span!("pipeline", tasks = plan.tasks.len());
        let pipeline_guard = _pipeline_span.enter();

        tracing::info!(
            requested_tasks = ?task_names,
            total_tasks = %plan.tasks.len(),
            levels = %plan.levels.len(),
            "Starting task execution pipeline"
        );

        // Execute tasks level by level
        for (level_idx, level) in plan.levels.iter().enumerate() {
            // TODO: Add tracing when moved to workspace
            let _level_span = tracing::info_span!("level", idx = level_idx, tasks = level.len());
            let level_guard = _level_span.enter();

            tracing::info!(
                level = %level_idx,
                tasks = ?level,
                "Starting execution level"
            );
            let mut join_set = JoinSet::new();
            let failed_tasks = Arc::new(Mutex::new(Vec::with_capacity(level.len())));

            // Launch all tasks in this level concurrently
            for task_name in level {
                let task_definition = match plan.tasks.get(task_name) {
                    Some(definition) => definition.clone(),
                    None => {
                        return Err(Error::configuration(format!(
                            "Task '{task_name}' not found in execution plan"
                        )));
                    }
                };

                // Determine working directory based on whether this is a cross-package task
                let working_dir = if let Some(ref registry) = self.monorepo_registry {
                    // For cross-package tasks, get the package path from the registry
                    if let Some(task) = registry.get_task(task_name) {
                        task.package_path.clone()
                    } else {
                        self.working_dir.clone()
                    }
                } else {
                    self.working_dir.clone()
                };

                super::task::spawn_task_execution(
                    &mut join_set,
                    super::task::TaskExecutionParams {
                        task_name: task_name.clone(),
                        task_definition,
                        working_dir,
                        task_args: args.to_vec(),
                        failed_tasks: Arc::clone(&failed_tasks),
                        action_cache: Arc::clone(&self.action_cache),
                        _env_manager: self.env_manager.clone(),
                        cache_config: self.cache_config.clone(),
                        executed_tasks: Arc::clone(&self.executed_tasks),
                        audit_mode,
                        capture_output,
                    },
                );
            }

            // Wait for all tasks in this level to complete
            while let Some(result) = join_set.join_next().await {
                if let Err(e) = result {
                    return Err(Error::configuration(format!("Task execution failed: {e}")));
                }
            }

            // Check if any tasks failed
            let failed = failed_tasks
                .lock()
                .map_err(|e| Error::configuration(format!("Failed to acquire lock: {e}")))?;
            if !failed.is_empty() {
                let failed_names: Vec<&str> =
                    failed.iter().map(|(name, _)| name.as_str()).collect();
                return Err(Error::configuration(format!(
                    "Tasks failed: {}",
                    failed_names.join(", ")
                )));
            }

            drop(level_guard);
        }

        drop(pipeline_guard);
        tracing::info!("Task execution pipeline completed successfully");
        Ok(0)
    }

    /// Execute tasks using the DAG system - this ensures consistent ordering  
    pub async fn execute_tasks_dag(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
        capture_output: bool,
    ) -> Result<i32> {
        // Build DAG
        let dag = self.build_dag(task_names)?;
        let levels = dag.get_execution_levels()?;

        tracing::info!(
            requested_tasks = ?task_names,
            total_tasks = %dag.get_flattened_tasks().len(),
            levels = %levels.len(),
            "Starting unified DAG task execution"
        );

        // Execute tasks level by level using the DAG
        for (level_idx, level) in levels.iter().enumerate() {
            tracing::info!(
                level = %level_idx,
                tasks = ?level,
                "Starting execution level"
            );
            let mut join_set = JoinSet::new();
            let failed_tasks = Arc::new(Mutex::new(Vec::with_capacity(level.len())));

            // Launch all tasks in this level concurrently
            for task_id in level {
                // Skip barrier tasks (they contain "__" in their names and don't need execution)
                if task_id.contains("__") {
                    tracing::debug!(task_id = %task_id, "Skipping barrier task");
                    continue;
                }

                // Get the task definition from the DAG
                let task_definition = match dag.get_task_definition(task_id) {
                    Some(definition) => definition.clone(),
                    None => {
                        return Err(Error::configuration(format!(
                            "Task '{task_id}' not found in unified DAG"
                        )));
                    }
                };

                // Determine working directory based on whether this is a cross-package task
                let working_dir = if let Some(ref registry) = self.monorepo_registry {
                    if let Some(task) = registry.get_task(task_id) {
                        task.package_path.clone()
                    } else {
                        self.working_dir.clone()
                    }
                } else {
                    self.working_dir.clone()
                };

                super::task::spawn_task_execution(
                    &mut join_set,
                    super::task::TaskExecutionParams {
                        task_name: task_id.clone(),
                        task_definition,
                        working_dir,
                        task_args: args.to_vec(),
                        failed_tasks: Arc::clone(&failed_tasks),
                        action_cache: Arc::clone(&self.action_cache),
                        _env_manager: self.env_manager.clone(),
                        cache_config: self.cache_config.clone(),
                        executed_tasks: Arc::clone(&self.executed_tasks),
                        audit_mode,
                        capture_output,
                    },
                );
            }

            // Wait for all tasks in this level to complete
            while let Some(result) = join_set.join_next().await {
                if let Err(e) = result {
                    return Err(Error::configuration(format!("Task execution failed: {e}")));
                }
            }

            // Check if any tasks failed
            let failed = failed_tasks
                .lock()
                .map_err(|e| Error::configuration(format!("Failed to acquire lock: {e}")))?;
            if !failed.is_empty() {
                let failed_names: Vec<&str> =
                    failed.iter().map(|(name, _)| name.as_str()).collect();
                return Err(Error::configuration(format!(
                    "Tasks failed: {}",
                    failed_names.join(", ")
                )));
            }

            tracing::info!(level = %level_idx, "Completed execution level");
        }

        tracing::info!("Completed unified DAG task execution");
        Ok(0)
    }
}
