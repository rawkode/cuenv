use crate::executor::cache;
use crate::executor::context::TaskExecutionContext;
use cuenv_cache::concurrent::action::ActionCache;
use cuenv_cache::config::CacheConfiguration;
use cuenv_core::TaskDefinition;
use cuenv_env::manager::EnvManager;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::task::JoinSet;
use tracing::Instrument;

/// Spawn a task execution
pub fn spawn_task_execution(
    join_set: &mut JoinSet<i32>,
    task_name: String,
    task_definition: TaskDefinition,
    working_dir: PathBuf,
    task_args: Vec<String>,
    failed_tasks: Arc<Mutex<Vec<(String, i32)>>>,
    action_cache: Arc<ActionCache>,
    env_manager: EnvManager,
    cache_config: CacheConfiguration,
    executed_tasks: Arc<Mutex<HashSet<String>>>,
    audit_mode: bool,
    capture_output: bool,
) {
    // Create task span
    // TODO: Add tracing when moved to workspace
    let task_span = tracing::info_span!("task", name = task_name.as_str());

    join_set.spawn(
        async move {
            execute_single_task_async(
                task_name,
                task_definition,
                working_dir,
                task_args,
                failed_tasks,
                action_cache,
                env_manager,
                cache_config,
                executed_tasks,
                audit_mode,
                capture_output,
            )
            .await
        }
        .instrument(task_span),
    );
}

async fn execute_single_task_async(
    task_name: String,
    task_definition: TaskDefinition,
    working_dir: PathBuf,
    task_args: Vec<String>,
    failed_tasks: Arc<Mutex<Vec<(String, i32)>>>,
    action_cache: Arc<ActionCache>,
    env_manager: EnvManager,
    cache_config: CacheConfiguration,
    executed_tasks: Arc<Mutex<HashSet<String>>>,
    audit_mode: bool,
    capture_output: bool,
) -> i32 {
    let start_time = Instant::now();

    // Publish task started event
    publish_task_started(&task_name).await;

    // Disabled: Detailed task configuration events (not essential for now)
    if false {
        publish_task_config_events(&task_name, &task_definition, &env_manager).await;
    }

    let ctx = TaskExecutionContext {
        cache_config: &cache_config,
        working_dir: &working_dir,
        action_cache: &action_cache,
        audit_mode,
        capture_output,
    };

    match cache::execute_single_task_with_cache(&ctx, &task_name, &task_definition, &task_args)
        .await
    {
        Ok(status) => {
            handle_task_success(status, &task_name, start_time, failed_tasks, executed_tasks).await
        }
        Err(e) => handle_task_error(e, &task_name, start_time, failed_tasks).await,
    }
}

async fn publish_task_started(task_name: &str) {
    let event_bus = cuenv_core::events::global_event_bus();
    let _ = event_bus
        .publish(cuenv_core::SystemEvent::Task(
            cuenv_core::TaskEvent::TaskStarted {
                task_name: task_name.to_string(),
                task_id: task_name.to_string(),
            },
        ))
        .await;
}

#[allow(dead_code)]
async fn publish_task_config_events(
    _task_name: &str,
    task_definition: &TaskDefinition,
    env_manager: &EnvManager,
) {
    // Show capabilities for this task's command
    if let cuenv_core::TaskExecutionMode::Command { command } = &task_definition.execution_mode {
        let capabilities = env_manager.get_command_capabilities(command);
        if !capabilities.is_empty() {
            // Event publishing code commented out for now
        }
    }

    if !task_definition.shell.is_empty() {
        // Event publishing code commented out for now
    }

    if task_definition.timeout.as_millis() > 0 {
        // Event publishing code commented out for now
    }

    if !task_definition.working_directory.as_os_str().is_empty() {
        // Event publishing code commented out for now
    }

    if let Some(security) = &task_definition.security {
        let mut restrictions = Vec::new();
        if security.restrict_disk {
            restrictions.push("disk");
        }
        if security.restrict_network {
            restrictions.push("network");
        }
        if !restrictions.is_empty() {
            // Event publishing code commented out for now
        }
    }
}

async fn handle_task_success(
    status: i32,
    task_name: &str,
    start_time: Instant,
    failed_tasks: Arc<Mutex<Vec<(String, i32)>>>,
    executed_tasks: Arc<Mutex<HashSet<String>>>,
) -> i32 {
    let duration_ms = start_time.elapsed().as_millis() as u64;

    if status != 0 {
        if let Ok(mut guard) = failed_tasks.lock() {
            guard.push((task_name.to_string(), status));
        } else {
            tracing::error!("Failed to acquire lock for failed tasks tracking");
        }

        // Publish task failed event
        let event_bus = cuenv_core::events::global_event_bus();
        let _ = event_bus
            .publish(cuenv_core::SystemEvent::Task(
                cuenv_core::TaskEvent::TaskFailed {
                    task_name: task_name.to_string(),
                    task_id: task_name.to_string(),
                    error: format!("Task exited with code {status}"),
                },
            ))
            .await;
    } else {
        // Mark task as executed
        if let Ok(mut guard) = executed_tasks.lock() {
            guard.insert(task_name.to_string());
        }

        // Publish task completed event
        let event_bus = cuenv_core::events::global_event_bus();
        let _ = event_bus
            .publish(cuenv_core::SystemEvent::Task(
                cuenv_core::TaskEvent::TaskCompleted {
                    task_name: task_name.to_string(),
                    task_id: task_name.to_string(),
                    duration_ms,
                },
            ))
            .await;

        tracing::info!(
            task = task_name,
            duration_ms = duration_ms,
            "Task completed"
        );
    }

    status
}

async fn handle_task_error(
    e: cuenv_core::Error,
    task_name: &str,
    start_time: Instant,
    failed_tasks: Arc<Mutex<Vec<(String, i32)>>>,
) -> i32 {
    let _duration_ms = start_time.elapsed().as_millis() as u64;

    if let Ok(mut guard) = failed_tasks.lock() {
        guard.push((task_name.to_string(), -1));
    } else {
        tracing::error!("Failed to acquire lock for failed tasks tracking");
    }

    // Publish task failed event
    let event_bus = cuenv_core::events::global_event_bus();
    let _ = event_bus
        .publish(cuenv_core::SystemEvent::Task(
            cuenv_core::TaskEvent::TaskFailed {
                task_name: task_name.to_string(),
                task_id: task_name.to_string(),
                error: e.to_string(),
            },
        ))
        .await;

    tracing::error!(
        task_name = %task_name,
        error = %e,
        "Task execution failed"
    );

    -1
}
