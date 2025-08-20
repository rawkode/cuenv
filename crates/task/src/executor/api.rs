use super::TaskExecutor;
use cuenv_core::{Error, Result};

impl TaskExecutor {
    /// Execute a single task by name
    pub async fn execute_task(&self, task_name: &str, args: &[String]) -> Result<i32> {
        self.execute_tasks_with_dependencies(&[task_name.to_string()], args, false)
            .await
    }

    /// Execute a single task by name with audit mode
    pub async fn execute_task_with_audit(&self, task_name: &str, args: &[String]) -> Result<i32> {
        self.execute_tasks_with_dependencies(&[task_name.to_string()], args, true)
            .await
    }

    /// Execute a task
    pub async fn execute(&mut self, task_name: &str) -> Result<()> {
        let exit_code = self.execute_task(task_name, &[]).await?;

        if exit_code != 0 {
            return Err(Error::configuration(format!(
                "Task '{task_name}' failed with exit code {exit_code}"
            )));
        }

        Ok(())
    }

    /// Get a topologically sorted list of tasks to execute
    pub fn get_execution_order(&self, task_name: &str) -> Result<Vec<String>> {
        let plan = self.build_execution_plan(&[task_name.to_string()])?;

        // Flatten the levels into a single list
        let mut order = Vec::new();
        for level in plan.levels {
            for task in level {
                order.push(task);
            }
        }

        Ok(order)
    }

    /// Check if a task has been executed (for testing)
    pub fn is_executed(&self, task_name: &str) -> bool {
        self.executed_tasks
            .lock()
            .map(|guard| guard.contains(task_name))
            .unwrap_or(false)
    }

    /// Execute multiple tasks with their dependencies
    pub async fn execute_tasks_with_dependencies(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32> {
        self.execute_tasks_with_dependencies_internal(task_names, args, audit_mode, false)
            .await
    }

    /// Execute multiple tasks with their dependencies and output capture
    pub async fn execute_tasks_with_capture(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32> {
        self.execute_tasks_with_dependencies_internal(task_names, args, audit_mode, true)
            .await
    }

    /// Execute tasks using the unified DAG system - ensures consistent ordering
    /// This is the new consolidated execution path that should be used going forward
    pub async fn execute_tasks_unified(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32> {
        self.execute_tasks_with_unified_dag(task_names, args, audit_mode)
            .await
    }
}
