//! Simple text output formatter using EventSubscriber pattern
//!
//! This formatter provides basic console output for task execution events.

use cuenv_core::events::{EnhancedEvent, EventSubscriber, SystemEvent, TaskEvent};
use std::io::{self, Write};

/// Simple formatter that outputs task events as plain text
pub struct SimpleFormatterSubscriber {
    trace_output: bool,
}

impl SimpleFormatterSubscriber {
    /// Create a new simple formatter
    pub fn new(trace_output: bool) -> Self {
        Self { trace_output }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for SimpleFormatterSubscriber {
    async fn handle_event(&self, event: &EnhancedEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let SystemEvent::Task(task_event) = &event.event {
            match task_event {
                TaskEvent::TaskStarted { task_name, .. } => {
                    println!("🚀 Starting task: {}", task_name);
                    if let Err(e) = io::stdout().flush() {
                        eprintln!("Warning: Failed to flush stdout: {}", e);
                    }
                }
                TaskEvent::TaskCompleted { task_name, duration_ms, .. } => {
                    let duration_sec = *duration_ms as f64 / 1000.0;
                    println!("✅ Completed task: {} ({:.2}s)", task_name, duration_sec);
                    if let Err(e) = io::stdout().flush() {
                        eprintln!("Warning: Failed to flush stdout: {}", e);
                    }
                }
                TaskEvent::TaskFailed { task_name, error, .. } => {
                    println!("❌ Failed task: {} - {}", task_name, error);
                    if let Err(e) = io::stdout().flush() {
                        eprintln!("Warning: Failed to flush stdout: {}", e);
                    }
                }
                TaskEvent::TaskProgress { task_name, message, .. } => {
                    if self.trace_output {
                        println!("⏳ Progress [{}]: {}", task_name, message);
                        if let Err(e) = io::stdout().flush() {
                            eprintln!("Warning: Failed to flush stdout: {}", e);
                        }
                    }
                }
                TaskEvent::TaskOutput { task_name, output, .. } => {
                    if self.trace_output {
                        println!("[{}] {}", task_name, output.trim_end());
                        if let Err(e) = io::stdout().flush() {
                            eprintln!("Warning: Failed to flush stdout: {}", e);
                        }
                    }
                }
                TaskEvent::TaskError { task_name, error, .. } => {
                    eprintln!("[{}] ERROR: {}", task_name, error.trim_end());
                    if let Err(e) = io::stderr().flush() {
                        eprintln!("Warning: Failed to flush stderr: {}", e);
                    }
                }
                TaskEvent::TaskSkipped { task_name, reason, .. } => {
                    println!("⏭️  Skipped task: {} ({})", task_name, reason);
                    if let Err(e) = io::stdout().flush() {
                        eprintln!("Warning: Failed to flush stdout: {}", e);
                    }
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "simple_formatter"
    }

    fn is_interested(&self, event: &SystemEvent) -> bool {
        // Simple formatter is interested in task events
        matches!(event, SystemEvent::Task(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_core::events::types::TaskEvent;
    use std::collections::HashMap;
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_simple_formatter_task_started() {
        let formatter = SimpleFormatterSubscriber::new(false);
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskStarted {
                task_name: "test-task".to_string(),
                task_id: "test-id".to_string(),
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        let result = formatter.handle_event(&event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_simple_formatter_task_completed() {
        let formatter = SimpleFormatterSubscriber::new(false);
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskCompleted {
                task_name: "test-task".to_string(),
                task_id: "test-id".to_string(),
                duration_ms: 1500,
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        let result = formatter.handle_event(&event).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_formatter_name() {
        let formatter = SimpleFormatterSubscriber::new(false);
        assert_eq!(formatter.name(), "simple_formatter");
    }

    #[test]
    fn test_is_interested_in_task_events() {
        let formatter = SimpleFormatterSubscriber::new(false);
        let task_event = SystemEvent::Task(TaskEvent::TaskStarted {
            task_name: "test".to_string(),
            task_id: "test-id".to_string(),
        });
        assert!(formatter.is_interested(&task_event));
    }
}