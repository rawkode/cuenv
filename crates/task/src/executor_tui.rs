use crate::executor::TaskExecutor;
use cuenv_core::{Error, Result};
// Temporary stubs to break circular dependency
#[derive(Clone)]
pub struct EventBus;

impl EventBus {
    pub fn new() -> Self {
        Self
    }

    pub fn set_global(_event_bus: Self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn publish_task_start(&self, _task_id: &str, _task_name: &str, _command: &str) {}
    pub fn publish_task_output(&self, _task_id: &str, _output: String, _is_error: bool) {}
    pub fn publish_task_complete(&self, _task_id: &str, _exit_code: i32, _duration_ms: u64) {}
    pub fn publish_task_failed(&self, _task_id: &str, _error: String, _duration_ms: u64) {}
}

pub struct FallbackRenderer;

impl FallbackRenderer {
    pub fn new(_event_bus: EventBus) -> Self {
        Self
    }

    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

pub struct TuiApp;

impl TuiApp {
    pub fn new(_event_bus: EventBus) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self)
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
use std::io::IsTerminal;
use std::sync::Arc;

/// Extension trait to add TUI support to TaskExecutor
pub trait TaskExecutorTui {
    /// Execute tasks with TUI visualization
    async fn execute_with_tui(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32>;

    /// Execute tasks with simple output (force fallback renderer)
    async fn execute_with_simple_output(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
        trace_output: bool,
    ) -> Result<i32>;

    /// Execute tasks with spinner output (Docker Compose-style)
    async fn execute_with_spinner(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32>;
}

impl TaskExecutorTui for TaskExecutor {
    async fn execute_with_tui(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32> {
        // Build execution plan
        let plan = self.build_execution_plan(task_names)?;

        // Create event bus
        let event_bus = EventBus::new();

        // Set as global event bus so task executor can publish events
        let _ = EventBus::set_global(event_bus.clone());

        // Register all tasks with the event bus
        for (task_name, task_config) in &plan.tasks {
            let dependencies = task_config.dependencies.clone().unwrap_or_default();
            event_bus
                .register_task(task_name.clone(), dependencies)
                .await;
        }

        // Check if we're in a TTY environment
        if std::io::stderr().is_terminal() {
            // Run with TUI
            let bus_clone = event_bus.clone();
            let executor_clone = self.clone();

            // Spawn task execution in parallel with TUI
            let self_clone = self.clone();
            let task_names_clone = task_names.to_vec();
            let args_clone = args.to_vec();
            let task_handle = tokio::spawn(async move {
                self_clone
                    .execute_tasks_with_dependencies_internal(
                        &task_names_clone,
                        &args_clone,
                        audit_mode,
                        true,
                    )
                    .await
            });

            // Spawn TUI in a separate task and keep it open until user quits (press 'q')
            let tui_handle = tokio::spawn(async move {
                // Set up panic handler to catch any panics in TUI
                let orig_hook = std::panic::take_hook();
                std::panic::set_hook(Box::new(move |panic_info| {
                    // Try to restore terminal before printing panic
                    let _ = crossterm::terminal::disable_raw_mode();
                    let _ = crossterm::execute!(
                        std::io::stderr(),
                        crossterm::terminal::LeaveAlternateScreen,
                        crossterm::event::DisableMouseCapture
                    );

                    // Log panic info to a file instead of stderr
                    let _ = std::fs::write(
                        "/tmp/cuenv-tui-panic.log",
                        format!("TUI PANIC: {}", panic_info),
                    );
                    orig_hook(panic_info);
                }));

                match TuiApp::new(bus_clone, executor_clone).await {
                    Ok(mut app) => {
                        let _ = app.run().await;
                    }
                    Err(_) => {
                        // TUI initialization failed
                    }
                }
            });

            // Wait for tasks to complete
            let task_result = task_handle.await.unwrap_or_else(|e| {
                Err(Error::configuration(format!(
                    "Task execution panicked: {}",
                    e
                )))
            });

            // Wait for TUI task (user quits with 'q') regardless of task success/failure
            // This ensures the user can inspect the results before the program exits
            let _ = tui_handle.await;

            // In TUI mode, we don't want to propagate task errors to the terminal
            // The user has already seen the failure in the TUI
            match task_result {
                Ok(exit_code) => Ok(exit_code),
                Err(_) => Ok(1), // Return non-zero exit code but don't print error
            }
        } else {
            // Non-TTY fallback (ASCII outline + Chrome Trace JSON)
            let fallback = FallbackRenderer::new(
                event_bus.registry().clone(),
                Some("cuenv-task-run".to_string()),
            );

            // Print initial DAG
            let dag = fallback.generate_ascii_dag(&plan).await;
            println!("{}", dag);

            // Subscribe to events
            let mut subscriber = event_bus.subscribe();
            let fallback_clone = Arc::new(fallback);
            let fc = fallback_clone.clone();

            // Spawn event handler
            let event_handle = tokio::spawn(async move {
                while let Some(event) = subscriber.recv().await {
                    fc.handle_event(event).await;
                }
            });

            // Execute tasks
            let result = self
                .execute_tasks_with_dependencies(task_names, args, audit_mode)
                .await;

            // Write output files after tasks complete
            let _ = fallback_clone.write_output_files(&plan).await;

            // Cleanup
            drop(event_bus);
            let _ = event_handle.await;

            result
        }
    }

    async fn execute_with_simple_output(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
        trace_output: bool,
    ) -> Result<i32> {
        // Build execution plan
        let plan = self.build_execution_plan(task_names)?;

        // Create event bus
        let event_bus = EventBus::new();

        // Set as global event bus so task executor can publish events
        let _ = EventBus::set_global(event_bus.clone());

        // Register all tasks with the event bus
        for (task_name, task_config) in &plan.tasks {
            let dependencies = task_config.dependencies.clone().unwrap_or_default();
            event_bus
                .register_task(task_name.clone(), dependencies)
                .await;
        }

        // Always use fallback renderer regardless of TTY
        let fallback = FallbackRenderer::new(
            event_bus.registry().clone(),
            if trace_output {
                Some("cuenv-task-run".to_string())
            } else {
                None
            },
        );

        // Print initial DAG
        let dag = fallback.generate_ascii_dag(&plan).await;
        println!("{}", dag);

        // Subscribe to events
        let mut subscriber = event_bus.subscribe();
        let fallback_clone = Arc::new(fallback);
        let fc = fallback_clone.clone();

        // Create a channel to signal when tasks are done
        let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn event handler
        let event_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = subscriber.recv() => {
                        if let Some(event) = event {
                            fc.handle_event(event).await;
                        } else {
                            break;
                        }
                    }
                    _ = &mut done_rx => {
                        break;
                    }
                }
            }
        });

        // Execute tasks
        let result = self
            .execute_tasks_with_dependencies(task_names, args, audit_mode)
            .await;

        // Write output files after tasks complete
        let _ = fallback_clone.write_output_files(&plan).await;

        // Signal event handler to stop
        let _ = done_tx.send(());

        // Cleanup
        drop(event_bus);
        let _ = event_handle.await;

        result
    }

    async fn execute_with_spinner(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32> {
        // Temporary stub for SpinnerFormatter
        pub struct SpinnerFormatter {
            event_bus: EventBus,
        }

        impl SpinnerFormatter {
            pub fn new(event_bus: EventBus) -> Self {
                Self { event_bus }
            }

            pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
                println!("Starting tasks...");
                Ok(())
            }
        }

        // Build execution plan
        let plan = self.build_execution_plan(task_names)?;

        // Create event bus
        let event_bus = EventBus::new();

        // Set as global event bus so task executor can publish events
        let _ = EventBus::set_global(event_bus.clone());

        // Register all tasks with the event bus
        for (task_name, task_config) in &plan.tasks {
            let dependencies = task_config.dependencies.clone().unwrap_or_default();
            event_bus
                .register_task(task_name.clone(), dependencies)
                .await;
        }

        // Create spinner formatter
        let mut formatter = SpinnerFormatter::new(event_bus.registry().clone());

        // Initialize the display
        formatter.initialize(&plan).await.map_err(|e| {
            Error::configuration(format!("Failed to initialize spinner formatter: {}", e))
        })?;

        // Subscribe to events
        let mut subscriber = event_bus.subscribe();
        let formatter = Arc::new(tokio::sync::RwLock::new(formatter));
        let formatter_clone = formatter.clone();

        // Create a channel to signal completion
        let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn event handler with completion signal
        let event_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = subscriber.recv() => {
                        if let Some(event) = event {
                            let formatter = formatter_clone.read().await;
                            let _ = formatter.handle_event(event).await;
                        } else {
                            break;
                        }
                    }
                    _ = &mut done_rx => {
                        break;
                    }
                }
            }
        });

        // Spawn ticker for animation
        let formatter_tick = formatter.clone();
        let ticker_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                let formatter = formatter_tick.read().await;
                if formatter.tick().await.is_err() {
                    break;
                }
            }
        });

        // Execute tasks
        let result = self
            .execute_tasks_with_dependencies_internal(task_names, args, audit_mode, true)
            .await;

        // Signal completion to event handler
        let _ = done_tx.send(());

        // Stop the ticker
        ticker_handle.abort();

        // Wait for event handler to finish
        let _ = event_handle.await;

        // Cleanup formatter
        let formatter = formatter.read().await;
        let _ = formatter.cleanup();

        // Cleanup event bus
        drop(event_bus);

        result
    }
}
