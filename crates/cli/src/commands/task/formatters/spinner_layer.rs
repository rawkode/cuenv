//! Spinner formatter layer for animated task display

use cuenv_tui::{SpinnerFormatter, TaskEvent, TaskRegistry};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};

/// Spinner formatter layer that provides animated task display
pub struct SpinnerFormatterLayer {
    formatter: Arc<Mutex<SpinnerFormatter>>,
    last_tick: Arc<Mutex<Instant>>,
    tick_interval: Duration,
}

impl SpinnerFormatterLayer {
    /// Create a new spinner formatter layer
    pub fn new() -> io::Result<Self> {
        let task_registry = TaskRegistry::new();
        let formatter = SpinnerFormatter::new(task_registry);

        Ok(Self {
            formatter: Arc::new(Mutex::new(formatter)),
            last_tick: Arc::new(Mutex::new(Instant::now())),
            tick_interval: Duration::from_millis(100),
        })
    }

    /// Create with custom tick interval
    pub fn with_tick_interval(interval: Duration) -> io::Result<Self> {
        let mut layer = Self::new()?;
        layer.tick_interval = interval;
        Ok(layer)
    }

    fn handle_task_event(&self, event: TaskEvent) -> io::Result<()> {
        // Since we're in a sync context but the formatter is async,
        // we'll use a simple print-based approach for now
        // In a real implementation, you'd want to use an async runtime
        // or send events through a channel to an async handler

        let _name = match &event {
            TaskEvent::Started { task_name, .. } => {
                tracing::info!("🔄 {} (started)", task_name);
                task_name
            }
            TaskEvent::Progress {
                task_name, message, ..
            } => {
                tracing::info!("⏳ {} - {}", task_name, message);
                task_name
            }
            TaskEvent::Completed {
                task_name,
                duration_ms,
                ..
            } => {
                tracing::info!("✅ {} (completed in {}ms)", task_name, duration_ms);
                task_name
            }
            TaskEvent::Failed {
                task_name, error, ..
            } => {
                tracing::error!("❌ {} (failed: {})", task_name, error);
                task_name
            }
            TaskEvent::Cancelled { task_name, .. } => {
                tracing::warn!("⏹️  {} (cancelled)", task_name);
                task_name
            }
            _ => return Ok(()),
        };

        // Simple tick for animation (simplified)
        // Note: In a proper implementation, this would update a progress indicator
        // through the tracing layer or a separate output mechanism
        let now = Instant::now();
        if let Ok(mut last_tick) = self.last_tick.lock() {
            if now.duration_since(*last_tick) >= self.tick_interval {
                tracing::debug!("Spinner tick for task animation");
                *last_tick = now;
            }
        }

        Ok(())
    }
}

impl Default for SpinnerFormatterLayer {
    fn default() -> Self {
        Self::new().expect("Failed to create SpinnerFormatterLayer")
    }
}

impl<S> Layer<S> for SpinnerFormatterLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Try to extract TaskEvent from the event
        if let Some(task_event) = extract_task_event(event) {
            if let Err(e) = self.handle_task_event(task_event) {
                eprintln!("SpinnerFormatterLayer error: {}", e);
            }
        }
    }
}

impl Drop for SpinnerFormatterLayer {
    fn drop(&mut self) {
        if let Ok(formatter) = self.formatter.lock() {
            if let Err(e) = formatter.cleanup() {
                eprintln!("Error cleaning up spinner formatter: {}", e);
            }
        }
    }
}

fn extract_task_event(event: &Event<'_>) -> Option<TaskEvent> {
    // Extract TUI TaskEvent from tracing event
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
