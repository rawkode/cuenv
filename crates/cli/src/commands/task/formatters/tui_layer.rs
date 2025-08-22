//! Full TUI formatter layer for rich interactive task display

use cuenv_task::executor::TaskExecutor;
use cuenv_tui::TuiApp;
use std::io;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};

/// TUI formatter layer that provides full interactive display
pub struct TuiFormatterLayer {
    _executor: Option<TaskExecutor>,
}

impl TuiFormatterLayer {
    /// Create a new TUI formatter layer
    pub fn new() -> io::Result<Self> {
        Ok(Self { _executor: None })
    }

    /// Set the task executor that will be used to launch the TUI
    pub fn with_executor(mut self, executor: TaskExecutor) -> Self {
        self._executor = Some(executor);
        self
    }

    /// Launch the TUI interface
    pub async fn launch_tui(&self) -> io::Result<()> {
        if let Some(executor) = &self._executor {
            // Clone the executor for the TUI
            let executor_clone = executor.clone();

            // Create and run the TUI app
            let mut tui_app = TuiApp::new_for_listing(executor_clone)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            tui_app
                .run()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        } else {
            // Fallback to simple output if no executor is available
            eprintln!("TUI mode requires a task executor. Falling back to simple output.");
        }
        Ok(())
    }
}

impl<S> Layer<S> for TuiFormatterLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // The TUI layer doesn't need to handle individual tracing events
        // because the TUI will be launched separately and will handle
        // events through its own event system
        let _ = event; // Suppress unused variable warning
    }
}
