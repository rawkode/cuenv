//! Full TUI formatter layer for rich interactive task display

use cuenv_tui::TaskEvent;
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};

/// TUI formatter layer that provides full interactive display
pub struct TuiFormatterLayer {
    // In a real implementation, this would integrate with a proper TUI library
    // like crossterm, tui-rs, or ratatui
    _state: Arc<Mutex<TuiState>>,
    last_update: Arc<Mutex<Instant>>,
    update_interval: Duration,
}

#[derive(Debug)]
struct TuiState {
    tasks: HashMap<String, TaskEvent>,
    is_initialized: bool,
    terminal_size: (u16, u16),
}

impl TuiFormatterLayer {
    /// Create a new TUI formatter layer
    pub fn new() -> io::Result<Self> {
        let state = TuiState {
            tasks: HashMap::new(),
            is_initialized: false,
            terminal_size: (80, 24), // Default size
        };

        Ok(Self {
            _state: Arc::new(Mutex::new(state)),
            last_update: Arc::new(Mutex::new(Instant::now())),
            update_interval: Duration::from_millis(50), // 20 FPS
        })
    }

    /// Create with custom update interval
    pub fn with_update_interval(interval: Duration) -> io::Result<Self> {
        let mut layer = Self::new()?;
        layer.update_interval = interval;
        Ok(layer)
    }

    fn handle_task_event(&self, event: TaskEvent) -> io::Result<()> {
        if let Ok(mut state) = self._state.lock() {
            let task_name = match &event {
                TaskEvent::Started { task_name, .. } => task_name.clone(),
                TaskEvent::Progress { task_name, .. } => task_name.clone(),
                TaskEvent::Log { task_name, .. } => task_name.clone(),
                TaskEvent::Completed { task_name, .. } => task_name.clone(),
                TaskEvent::Failed { task_name, .. } => task_name.clone(),
                TaskEvent::Cancelled { task_name, .. } => task_name.clone(),
            };

            state.tasks.insert(task_name, event);

            // Check if we should update the display
            let now = Instant::now();
            if let Ok(mut last_update) = self.last_update.lock() {
                if now.duration_since(*last_update) >= self.update_interval {
                    self.render_ui(&state)?;
                    *last_update = now;
                }
            }
        }
        Ok(())
    }

    fn render_ui(&self, _state: &TuiState) -> io::Result<()> {
        // In a real implementation, this would:
        // 1. Clear the terminal or update specific areas
        // 2. Draw a proper UI with:
        //    - Task hierarchy display
        //    - Progress bars
        //    - Real-time status updates
        //    - Logs panel
        //    - Controls/help
        // 3. Use a proper TUI library for efficient rendering

        // For now, just a placeholder that doesn't actually render anything
        // to avoid interfering with other formatters
        Ok(())
    }
}

impl Default for TuiFormatterLayer {
    fn default() -> Self {
        Self::new().expect("Failed to create TuiFormatterLayer")
    }
}

impl<S> Layer<S> for TuiFormatterLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Try to extract TaskEvent from the event
        if let Some(task_event) = extract_task_event(event) {
            if let Err(e) = self.handle_task_event(task_event) {
                eprintln!("TuiFormatterLayer error: {}", e);
            }
        }
    }
}

impl Drop for TuiFormatterLayer {
    fn drop(&mut self) {
        // In a real implementation, this would:
        // 1. Restore terminal state
        // 2. Show cursor
        // 3. Clear any remaining UI elements
        // 4. Reset terminal modes
    }
}

fn extract_task_event(event: &Event<'_>) -> Option<TaskEvent> {
    // Extract TUI TaskEvent from tracing event - same as spinner layer
    let mut task_name = None;
    let mut event_type = None;
    let mut message = None;
    let mut error_msg = None;
    let mut duration_ms = None;

    event.record(
        &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| match field.name() {
            "task_name" => task_name = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "event_type" => event_type = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "message" => message = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "error" => error_msg = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "duration_ms" => duration_ms = format!("{:?}", value).parse().ok(),
            _ => {}
        },
    );

    let task_name = task_name?;
    let timestamp = std::time::Instant::now();

    match event_type?.as_str() {
        "started" => Some(TaskEvent::Started {
            task_name,
            timestamp,
        }),
        "progress" => Some(TaskEvent::Progress {
            task_name,
            message: message.unwrap_or_else(|| "Progress update".to_string()),
        }),
        "completed" => Some(TaskEvent::Completed {
            task_name,
            exit_code: 0,
            duration_ms: duration_ms.unwrap_or(0),
        }),
        "failed" => Some(TaskEvent::Failed {
            task_name,
            error: error_msg.unwrap_or_else(|| "Unknown error".to_string()),
            duration_ms: duration_ms.unwrap_or(0),
        }),
        "cancelled" => Some(TaskEvent::Cancelled { task_name }),
        _ => None,
    }
}
