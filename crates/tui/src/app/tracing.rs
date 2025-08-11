use crate::events::TaskEvent;
use std::time::Instant;

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
            self.duration_ms = Some(value)
        }
    }

    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {
        // Handle debug fields if needed
    }
}