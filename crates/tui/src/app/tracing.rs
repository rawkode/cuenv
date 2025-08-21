use crate::events::TaskEvent;
use crate::events::{TracingEvent, TracingLevel};
use chrono::Local;
use std::time::Instant;
use tracing::{Event, Metadata};

/// Convert tracing events to TUI events
pub fn tracing_to_tui_event(
    event: &tracing::Event,
    metadata: &tracing::Metadata,
) -> Option<TaskEvent> {
    let mut visitor = EventVisitor::default();
    event.record(&mut visitor);

    match metadata.name() {
        "task_started" => Some(TaskEvent::Started {
            task_name: visitor.task_name?,
            timestamp: Instant::now(),
        }),
        "task_progress" => Some(TaskEvent::Progress {
            task_name: visitor.task_name?,
            message: visitor.message.unwrap_or_default(),
        }),
        "task_completed" => Some(TaskEvent::Completed {
            task_name: visitor.task_name?,
            exit_code: 0,
            duration_ms: visitor.duration_ms.unwrap_or(0),
        }),
        "task_failed" => Some(TaskEvent::Failed {
            task_name: visitor.task_name?,
            error: visitor
                .message
                .unwrap_or_else(|| "Unknown error".to_string()),
            duration_ms: visitor.duration_ms.unwrap_or(0),
        }),
        _ => None,
    }
}

/// Convert tracing events to general tracing output
pub fn tracing_to_general_event(event: &Event, metadata: &Metadata) -> Option<TracingEvent> {
    let mut visitor = EventVisitor::default();
    event.record(&mut visitor);

    // Convert tracing level to our internal level
    let level = match *metadata.level() {
        tracing::Level::TRACE => TracingLevel::Trace,
        tracing::Level::DEBUG => TracingLevel::Debug,
        tracing::Level::INFO => TracingLevel::Info,
        tracing::Level::WARN => TracingLevel::Warn,
        tracing::Level::ERROR => TracingLevel::Error,
    };

    // Extract message from the event
    let message = visitor.message.unwrap_or_else(|| {
        // If no explicit message field, try to format the event
        format!("{:?}", event)
    });

    // Collect any additional fields
    let mut fields = Vec::new();
    if let Some(task_name) = visitor.task_name {
        fields.push(("task".to_string(), task_name));
    }
    if let Some(duration) = visitor.duration_ms {
        fields.push(("duration_ms".to_string(), duration.to_string()));
    }

    Some(TracingEvent {
        timestamp: Local::now(),
        level,
        target: metadata.target().to_string(),
        message,
        fields,
    })
}

/// Check if a tracing event is task-specific
pub fn is_task_event(metadata: &Metadata) -> bool {
    matches!(
        metadata.name(),
        "task_started" | "task_progress" | "task_completed" | "task_failed"
    )
}

#[derive(Default)]
struct EventVisitor {
    task_name: Option<String>,
    message: Option<String>,
    duration_ms: Option<u64>,
}

impl tracing::field::Visit for EventVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "task_name" => self.task_name = Some(value.to_string()),
            "message" => self.message = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "duration_ms" {
            self.duration_ms = Some(value);
        }
    }

    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {
        // Handle debug fields if needed
    }
}

use crate::events::TaskRegistry;
use std::sync::Arc;
use tracing::Subscriber;
use tracing_subscriber::{layer::Context, Layer};

/// A tracing layer that routes events to the TUI TaskRegistry
pub struct TuiTracingLayer {
    task_registry: Arc<TaskRegistry>,
}

impl TuiTracingLayer {
    pub fn new(task_registry: Arc<TaskRegistry>) -> Self {
        Self { task_registry }
    }
}

impl<S> Layer<S> for TuiTracingLayer
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Extract task event from tracing event (copy from spinner_layer.rs)
        if let Some(task_event) = extract_task_event(event) {
            // Handle async in a blocking context
            let registry = self.task_registry.clone();
            tokio::spawn(async move {
                match task_event {
                    TaskEvent::Started { task_name, .. } => {
                        registry
                            .update_task_state(&task_name, crate::events::TaskState::Running)
                            .await;

                        registry
                            .add_log(
                                &task_name,
                                crate::events::LogStream::System,
                                "Task started".to_string(),
                            )
                            .await;
                    }
                    TaskEvent::Completed {
                        task_name,
                        exit_code,
                        duration_ms,
                    } => {
                        registry
                            .update_task_state(&task_name, crate::events::TaskState::Completed)
                            .await;

                        let log_message = format!(
                            "Task completed (exit code: {}, duration: {}ms)",
                            exit_code, duration_ms
                        );
                        registry
                            .add_log(&task_name, crate::events::LogStream::System, log_message)
                            .await;
                    }
                    TaskEvent::Failed {
                        task_name,
                        error,
                        duration_ms,
                    } => {
                        registry
                            .update_task_state(&task_name, crate::events::TaskState::Failed)
                            .await;

                        let log_message =
                            format!("Task failed: {} (duration: {}ms)", error, duration_ms);
                        registry
                            .add_log(&task_name, crate::events::LogStream::Stderr, log_message)
                            .await;
                    }
                    TaskEvent::Progress { task_name, message } => {
                        registry
                            .add_log(&task_name, crate::events::LogStream::Stdout, message)
                            .await;
                    }
                    TaskEvent::Cancelled { task_name } => {
                        registry
                            .update_task_state(&task_name, crate::events::TaskState::Cancelled)
                            .await;

                        registry
                            .add_log(
                                &task_name,
                                crate::events::LogStream::System,
                                "Task cancelled".to_string(),
                            )
                            .await;
                    }
                    TaskEvent::Log { .. } => {
                        // Log events are already handled, no additional processing needed
                    }
                }
            });
        }
    }
}

/// Extract task events from tracing events (copied from spinner_layer.rs)
fn extract_task_event(event: &Event<'_>) -> Option<TaskEvent> {
    let mut task_name = None;
    let mut event_type = None;
    let mut message = None;
    let mut error_msg = None;
    let mut duration_ms = None;

    event.record(
        &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| match field.name() {
            "task_name" => task_name = Some(format!("{value:?}").trim_matches('"').to_string()),
            "event_type" => event_type = Some(format!("{value:?}").trim_matches('"').to_string()),
            "message" => message = Some(format!("{value:?}").trim_matches('"').to_string()),
            "error" => error_msg = Some(format!("{value:?}").trim_matches('"').to_string()),
            "duration_ms" => duration_ms = format!("{value:?}").parse().ok(),
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
            message: message.unwrap_or_default(),
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
