use crate::events::TaskEvent;
use std::time::Instant;
use tracing::Event;

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
        "task_output" => Some(TaskEvent::Log {
            task_name: visitor.task_name?,
            stream: crate::events::LogStream::Stdout,
            content: visitor.output.unwrap_or_default(),
        }),
        "task_error" => Some(TaskEvent::Log {
            task_name: visitor.task_name?,
            stream: crate::events::LogStream::Stderr,
            content: visitor.error.unwrap_or_default(),
        }),
        _ => None,
    }
}

#[derive(Default)]
pub struct EventVisitor {
    pub task_name: Option<String>,
    pub message: Option<String>,
    pub duration_ms: Option<u64>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub fields: std::collections::HashMap<String, String>,
}

impl tracing::field::Visit for EventVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "task_name" => self.task_name = Some(value.to_string()),
            "message" => self.message = Some(value.to_string()),
            "output" => self.output = Some(value.to_string()),
            "error" => self.error = Some(value.to_string()),
            _ => {
                self.fields
                    .insert(field.name().to_string(), value.to_string());
            }
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "duration_ms" {
            self.duration_ms = Some(value);
        }
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{:?}", value));
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

impl TuiTracingLayer {}

impl<S> Layer<S> for TuiTracingLayer
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Extract task event from tracing event using the same logic as tracing_to_tui_event
        if let Some(task_event) = tracing_to_tui_event(event, event.metadata()) {
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
                    TaskEvent::Log {
                        task_name,
                        stream,
                        content,
                    } => {
                        registry.add_log(&task_name, stream, content).await;
                    }
                }
            });
        }
    }
}
