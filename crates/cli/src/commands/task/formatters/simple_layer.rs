//! Simple text formatter layer for basic task output

use cuenv_core::events::TaskEvent;
use std::io::{self, Write};
use std::sync::Mutex;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};

/// Simple formatter layer that outputs basic task status
pub struct SimpleFormatterLayer {
    writer: Mutex<Box<dyn Write + Send + Sync>>,
}

impl SimpleFormatterLayer {
    /// Create a new simple formatter writing to stdout
    pub fn new() -> Self {
        Self {
            writer: Mutex::new(Box::new(io::stdout())),
        }
    }

    /// Create a new simple formatter with custom writer
    pub fn with_writer<W: Write + Send + Sync + 'static>(writer: W) -> Self {
        Self {
            writer: Mutex::new(Box::new(writer)),
        }
    }

    fn format_task_event(&self, event: &TaskEvent) -> io::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        match event {
            TaskEvent::TaskStarted { task_name, .. } => {
                writeln!(writer, "🔄 {} (started)", task_name)?;
            }
            TaskEvent::TaskCompleted {
                task_name,
                duration_ms,
                ..
            } => {
                writeln!(writer, "✅ {} (completed in {}ms)", task_name, duration_ms)?;
            }
            TaskEvent::TaskFailed {
                task_name, error, ..
            } => {
                writeln!(writer, "❌ {} (failed)", task_name)?;
                writeln!(writer, "   Error: {}", error)?;
            }
            TaskEvent::TaskProgress {
                task_name, message, ..
            } => {
                writeln!(writer, "⏳ {} {}", task_name, message)?;
            }
            TaskEvent::TaskOutput {
                task_name, output, ..
            } => {
                writeln!(writer, "📝 {}: {}", task_name, output)?;
            }
            TaskEvent::TaskError {
                task_name, error, ..
            } => {
                writeln!(writer, "⚠️  {}: {}", task_name, error)?;
            }
            TaskEvent::TaskSkipped {
                task_name, reason, ..
            } => {
                writeln!(writer, "⏭️  {} (skipped: {})", task_name, reason)?;
            }
        }
        writer.flush()
    }

    fn format_generic_event(&self, event: &tracing::Event<'_>) -> io::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        // For generic events, just print the message with appropriate level indicator
        let level_indicator = match *event.metadata().level() {
            tracing::Level::ERROR => "❌",
            tracing::Level::WARN => "⚠️ ",
            tracing::Level::INFO => "",
            tracing::Level::DEBUG => "🐛",
            tracing::Level::TRACE => "🔍",
        };

        // Extract the message from the event
        let mut message = String::new();
        event.record(
            &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
                if field.name() == "message" {
                    message = format!("{:?}", value).trim_matches('"').to_string();
                }
            },
        );

        // If no explicit message field, use the target or default message
        if message.is_empty() {
            message = event.metadata().target().to_string();
        }

        writeln!(writer, "{}{}", level_indicator, message)?;
        writer.flush()
    }
}

impl Default for SimpleFormatterLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for SimpleFormatterLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Try to extract TaskEvent from the event first
        if let Some(task_event) = extract_task_event(event) {
            if let Err(e) = self.format_task_event(&task_event) {
                eprintln!("SimpleFormatterLayer write error: {}", e);
            }
        } else {
            // Handle generic tracing events (from CLI calls)
            if let Err(e) = self.format_generic_event(event) {
                eprintln!("SimpleFormatterLayer write error: {}", e);
            }
        }
    }
}

fn extract_task_event(event: &Event<'_>) -> Option<TaskEvent> {
    // Extract task event information from tracing event
    let mut task_name = None;
    let mut task_id = None;
    let mut event_type = None;
    let mut duration_ms = None;
    let mut error_msg = None;
    let mut message = None;
    let mut output = None;
    let mut reason = None;

    event.record(
        &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| match field.name() {
            "task_name" => task_name = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "task_id" => task_id = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "event_type" => event_type = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "duration_ms" => duration_ms = format!("{:?}", value).parse().ok(),
            "error" => error_msg = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "message" => message = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "output" => output = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "reason" => reason = Some(format!("{:?}", value).trim_matches('"').to_string()),
            _ => {}
        },
    );

    let task_name = task_name?;
    let task_id = task_id.unwrap_or_else(|| format!("{}-{}", task_name, std::process::id()));

    match event_type?.as_str() {
        "started" => Some(TaskEvent::TaskStarted { task_name, task_id }),
        "completed" => Some(TaskEvent::TaskCompleted {
            task_name,
            task_id,
            duration_ms: duration_ms.unwrap_or(0),
        }),
        "failed" => Some(TaskEvent::TaskFailed {
            task_name,
            task_id,
            error: error_msg.unwrap_or_else(|| "Unknown error".to_string()),
        }),
        "progress" => Some(TaskEvent::TaskProgress {
            task_name,
            task_id,
            message: message.unwrap_or_else(|| "Progress update".to_string()),
        }),
        "output" => Some(TaskEvent::TaskOutput {
            task_name,
            task_id,
            output: output.unwrap_or_else(|| "".to_string()),
        }),
        "error_output" => Some(TaskEvent::TaskError {
            task_name,
            task_id,
            error: error_msg.unwrap_or_else(|| "".to_string()),
        }),
        "skipped" => Some(TaskEvent::TaskSkipped {
            task_name,
            task_id,
            reason: reason.unwrap_or_else(|| "Unknown reason".to_string()),
        }),
        _ => None,
    }
}
