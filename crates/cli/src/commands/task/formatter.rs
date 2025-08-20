//! Task output formatters integration
//!
//! This module provides integration between the task executor and the TUI formatters.

use cuenv_core::Result;
use cuenv_task::TaskExecutor;
use cuenv_tui::app::TuiApp;
use cuenv_tui::event_bus::EventBus;
use cuenv_tui::events::{TaskRegistry, TaskState};
use cuenv_tui::spinner::SpinnerFormatter;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

/// Execute tasks with the appropriate output formatter
pub async fn execute_with_formatter(
    executor: &TaskExecutor,
    task_name: &str,
    args: &[String],
    audit: bool,
    output_format: &str,
    trace_output: bool,
) -> Result<i32> {
    // Set up signal handling for Ctrl-C
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    // Install signal handler
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            eprintln!("\n⚠️  Received interrupt signal, stopping tasks...");
            let _ = shutdown_tx_clone.send(()).await;
        }
    });

    match output_format {
        "spinner" => execute_with_spinner(executor, task_name, args, audit, &mut shutdown_rx).await,
        "simple" | "tree" => {
            execute_with_simple(
                executor,
                task_name,
                args,
                audit,
                trace_output,
                &mut shutdown_rx,
            )
            .await
        }
        "tui" => {
            // Check if we're in a TTY environment
            if !atty::is(atty::Stream::Stderr) {
                eprintln!(
                    "TUI mode requires an interactive terminal. Falling back to spinner mode."
                );
                execute_with_spinner(executor, task_name, args, audit, &mut shutdown_rx).await
            } else {
                // Use the full interactive TUI
                execute_with_tui(executor, task_name, args, audit, &mut shutdown_rx).await
            }
        }
        _ => {
            // Fall back to simple output for unknown formats
            eprintln!("Unknown output format '{output_format}', using simple output");
            execute_with_simple(
                executor,
                task_name,
                args,
                audit,
                trace_output,
                &mut shutdown_rx,
            )
            .await
        }
    }
}

/// Execute with spinner output (Docker Compose style)
async fn execute_with_spinner(
    executor: &TaskExecutor,
    task_name: &str,
    args: &[String],
    audit: bool,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> Result<i32> {
    // Create task registry for communication
    let task_registry = TaskRegistry::new();

    // Create spinner formatter
    let mut formatter = SpinnerFormatter::new(task_registry.clone());

    // Build unified DAG for this single task
    let dag = executor.build_unified_dag(&[task_name.to_string()])?;
    let levels = dag.get_execution_levels()?;

    // Create a compatible execution plan for the formatter
    let mut plan_tasks = std::collections::HashMap::new();
    for task in dag.get_flattened_tasks() {
        if !task.is_barrier {
            if let Some(definition) = dag.get_task_definition(&task.id) {
                plan_tasks.insert(task.id.clone(), definition.clone());
            }
        }
    }
    let plan = cuenv_task::TaskExecutionPlan {
        levels,
        tasks: plan_tasks,
    };

    // Initialize formatter with the plan
    formatter
        .initialize(&plan)
        .await
        .map_err(|e| cuenv_core::Error::Configuration {
            message: format!("Failed to initialize spinner: {e}"),
        })?;

    // Register all tasks in the execution plan with the registry
    for task_name_in_plan in plan.tasks.keys() {
        task_registry
            .register_task(task_name_in_plan.clone(), vec![])
            .await;
        task_registry
            .update_task_state(task_name_in_plan, TaskState::Queued)
            .await;
    }

    // We need to get the formatter as Arc before spawning tasks
    let formatter_arc = Arc::new(formatter);

    // Create a bridge to forward core events to spinner
    let formatter_for_bridge = formatter_arc.clone();
    let bridge_handle = tokio::spawn(async move {
        let core_bus = cuenv_core::events::global_event_bus();
        let mut subscriber = core_bus.subscribe();

        loop {
            match subscriber.recv().await {
                Ok(enhanced_event) => {
                    // Convert core task events to TUI events and send to formatter
                    if let cuenv_core::SystemEvent::Task(task_event) = enhanced_event.event {
                        let tui_event = match task_event {
                            cuenv_core::TaskEvent::TaskStarted { task_name, .. } => {
                                Some(cuenv_tui::TaskEvent::Started {
                                    task_name,
                                    timestamp: std::time::Instant::now(),
                                })
                            }
                            cuenv_core::TaskEvent::TaskCompleted {
                                task_name,
                                duration_ms,
                                ..
                            } => Some(cuenv_tui::TaskEvent::Completed {
                                task_name,
                                exit_code: 0,
                                duration_ms,
                            }),
                            cuenv_core::TaskEvent::TaskFailed {
                                task_name, error, ..
                            } => Some(cuenv_tui::TaskEvent::Failed {
                                task_name,
                                error,
                                duration_ms: 0,
                            }),
                            _ => None,
                        };

                        if let Some(event) = tui_event {
                            let _ = formatter_for_bridge.handle_event(event).await;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    // Start the spinner ticker in background
    let formatter_clone = formatter_arc.clone();
    let spinner_handle = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(100));
        loop {
            ticker.tick().await;
            if formatter_clone.tick().await.is_err() {
                break;
            }
        }
    });

    // Execute the task with cancellation support and OUTPUT CAPTURE
    // We use the internal method to enable capture_output for spinner mode
    let result = tokio::select! {
        result = async {
            // Use unified DAG execution (temporarily without output capture)
            executor.execute_tasks_unified(
                &[task_name.to_string()],
                args,
                audit
            ).await
        } => result,
        _ = shutdown_rx.recv() => {
            task_registry.update_task_state(task_name, TaskState::Cancelled).await;
            eprintln!("Task execution cancelled");
            Ok(130) // Standard exit code for SIGINT
        }
    };

    // Update final state
    match result {
        Ok(0) => {
            task_registry
                .update_task_state(task_name, TaskState::Completed)
                .await;
        }
        Ok(_) | Err(_) => {
            task_registry
                .update_task_state(task_name, TaskState::Failed)
                .await;
            // Note: Output will be shown by the executor when task fails
        }
    }

    // Stop the spinner and event bridge, cleanup BEFORE showing any error output
    spinner_handle.abort();
    bridge_handle.abort();
    let _ = formatter_arc.cleanup();

    // The error output (if any) will be printed by the executor after spinner is cleaned up
    // This ensures the error message appears after the spinner, not during it

    result
}

/// Execute with full interactive TUI
async fn execute_with_tui(
    executor: &TaskExecutor,
    task_name: &str,
    args: &[String],
    audit: bool,
    _shutdown_rx: &mut mpsc::Receiver<()>,
) -> Result<i32> {
    // Create event bus for the TUI
    let event_bus = EventBus::new();

    // Get the task registry from the event bus
    let task_registry = event_bus.registry();

    // Register the task to be executed
    task_registry
        .register_task(task_name.to_string(), vec![])
        .await;

    // Create a bridge to forward core events to TUI event bus
    let tui_event_bus = event_bus.clone();
    let bridge_handle = tokio::spawn(async move {
        let core_bus = cuenv_core::events::global_event_bus();
        let mut subscriber = core_bus.subscribe();

        loop {
            match subscriber.recv().await {
                Ok(enhanced_event) => {
                    // Convert core task events to TUI events
                    if let cuenv_core::SystemEvent::Task(task_event) = enhanced_event.event {
                        match task_event {
                            cuenv_core::TaskEvent::TaskStarted { task_name, .. } => {
                                tui_event_bus
                                    .publish(cuenv_tui::events::TaskEvent::Started {
                                        task_name,
                                        timestamp: std::time::Instant::now(),
                                    })
                                    .await;
                            }
                            cuenv_core::TaskEvent::TaskCompleted {
                                task_name,
                                duration_ms,
                                ..
                            } => {
                                tui_event_bus
                                    .publish(cuenv_tui::events::TaskEvent::Completed {
                                        task_name,
                                        exit_code: 0,
                                        duration_ms,
                                    })
                                    .await;
                            }
                            cuenv_core::TaskEvent::TaskFailed {
                                task_name, error, ..
                            } => {
                                tui_event_bus
                                    .publish(cuenv_tui::events::TaskEvent::Failed {
                                        task_name: task_name.clone(),
                                        error,
                                        duration_ms: 0, // Duration not available from core event
                                    })
                                    .await;
                            }
                            cuenv_core::TaskEvent::TaskOutput {
                                task_name, output, ..
                            } => {
                                tui_event_bus
                                    .publish(cuenv_tui::events::TaskEvent::Log {
                                        task_name,
                                        stream: cuenv_tui::events::LogStream::Stdout,
                                        content: output,
                                    })
                                    .await;
                            }
                            cuenv_core::TaskEvent::TaskError {
                                task_name, error, ..
                            } => {
                                tui_event_bus
                                    .publish(cuenv_tui::events::TaskEvent::Log {
                                        task_name,
                                        stream: cuenv_tui::events::LogStream::Stderr,
                                        content: error,
                                    })
                                    .await;
                            }
                            // Forward other events if needed
                            _ => {}
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    // Create the TUI app
    let mut tui_app = TuiApp::new(event_bus.clone(), executor.clone())
        .await
        .map_err(|e| cuenv_core::Error::Configuration {
            message: format!("Failed to create TUI app: {e}"),
        })?;

    // Start task execution in the background
    let executor_clone = executor.clone();
    let task_name_clone = task_name.to_string();
    let args_clone = args.to_vec();
    let task_handle = tokio::spawn(async move {
        // Small delay to let TUI initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Execute the task with unified DAG (temporarily without output capture)
        executor_clone
            .execute_tasks_unified(&[task_name_clone], &args_clone, audit)
            .await
    });

    // Run the TUI (this blocks until user quits)
    let tui_result = tui_app.run().await;

    // Stop the event bridge
    bridge_handle.abort();

    // Get the task result
    let task_result = match task_handle.await {
        Ok(result) => result,
        Err(_) => Ok(1), // Task was cancelled or panicked
    };

    // Check for TUI errors
    if let Err(e) = tui_result {
        eprintln!("TUI error: {e}");
    }

    task_result
}

/// Execute with simple/fallback output
async fn execute_with_simple(
    executor: &TaskExecutor,
    task_name: &str,
    args: &[String],
    audit: bool,
    trace_output: bool,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> Result<i32> {
    if trace_output {
        eprintln!("Note: Chrome trace output is not yet implemented");
    }

    // Build unified DAG to show all tasks that will be executed (including dependencies)
    let dag = executor.build_unified_dag(&[task_name.to_string()])?;
    let levels = dag.get_execution_levels()?;

    // Show all tasks that will be executed
    let all_task_count = dag
        .get_flattened_tasks()
        .iter()
        .filter(|t| !t.is_barrier)
        .count();
    if all_task_count > 1 {
        println!(
            "Executing task: {task_name} (with {} dependencies)",
            all_task_count - 1
        );
        for level in &levels {
            for task_id in level {
                if !task_id.contains("__") && task_id != task_name {
                    // Skip barriers and main task
                    println!("Executing dependency: {task_id}");
                }
            }
        }
    } else {
        println!("Executing task: {task_name}");
    }

    if !args.is_empty() {
        println!("Arguments: {args:?}");
    }

    // Execute with cancellation support - use unified DAG to ensure consistent ordering
    let result = tokio::select! {
        result = async {
            if audit {
                println!("Running in audit mode...");
                executor.execute_tasks_unified(&[task_name.to_string()], args, audit).await
            } else {
                executor.execute_tasks_unified(&[task_name.to_string()], args, audit).await
            }
        } => result,
        _ = shutdown_rx.recv() => {
            eprintln!("\n⚠️  Task cancelled by user");
            Ok(130) // Standard exit code for SIGINT
        }
    };

    match result {
        Ok(0) => {
            println!("✓ Task completed successfully");
        }
        Ok(130) => {
            // Don't print extra message for cancellation
        }
        Ok(code) => {
            eprintln!("✗ Task failed with exit code: {code}");
        }
        Err(ref e) => {
            eprintln!("✗ Task failed: {e}");
        }
    }

    result
}
