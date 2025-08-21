//! TUI output formatter using EventSubscriber pattern
//!
//! This formatter provides full interactive TUI for task execution events.

use cuenv_core::events::{EnhancedEvent, EventSubscriber, SystemEvent, TaskEvent};
use cuenv_tui::app::TuiApp;
use cuenv_tui::event_bus::EventBus;
use cuenv_tui::events::TaskRegistry;
use std::sync::Arc;

/// TUI formatter that provides full interactive terminal UI for task events
pub struct TuiFormatterSubscriber {
    event_bus: EventBus,
    task_registry: TaskRegistry,
    tui_app: Option<Arc<TuiApp>>,
}

impl TuiFormatterSubscriber {
    /// Create a new TUI formatter
    pub fn new() -> Self {
        let event_bus = EventBus::new();
        let task_registry = event_bus.registry().clone();
        
        Self {
            event_bus,
            task_registry,
            tui_app: None,
        }
    }

    /// Initialize the TUI application
    pub async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create TUI app
        let app = TuiApp::new(self.event_bus.clone())?;
        self.tui_app = Some(Arc::new(app));
        Ok(())
    }

    /// Start the TUI application
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(app) = &self.tui_app {
            app.run().await?;
        }
        Ok(())
    }

    /// Register a task in the TUI
    pub async fn register_task(&self, name: String, dependencies: Vec<String>) {
        self.task_registry.register_task(name, dependencies).await;
    }
}

impl Default for TuiFormatterSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventSubscriber for TuiFormatterSubscriber {
    async fn handle_event(&self, event: &EnhancedEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let SystemEvent::Task(task_event) = &event.event {
            // Convert core task events to TUI events and publish to the event bus
            match task_event {
                TaskEvent::TaskStarted { task_name, .. } => {
                    self.event_bus
                        .publish(cuenv_tui::events::TaskEvent::Started {
                            task_name: task_name.clone(),
                            timestamp: std::time::Instant::now(),
                        })
                        .await;
                }
                TaskEvent::TaskCompleted { task_name, duration_ms, .. } => {
                    self.event_bus
                        .publish(cuenv_tui::events::TaskEvent::Completed {
                            task_name: task_name.clone(),
                            exit_code: 0,
                            duration_ms: *duration_ms,
                        })
                        .await;
                }
                TaskEvent::TaskFailed { task_name, error, .. } => {
                    self.event_bus
                        .publish(cuenv_tui::events::TaskEvent::Failed {
                            task_name: task_name.clone(),
                            error: error.clone(),
                            duration_ms: 0, // Duration not available from core event
                        })
                        .await;
                }
                TaskEvent::TaskProgress { task_name, message, .. } => {
                    self.event_bus
                        .publish(cuenv_tui::events::TaskEvent::Progress {
                            task_name: task_name.clone(),
                            message: message.clone(),
                        })
                        .await;
                }
                TaskEvent::TaskOutput { task_name, output, .. } => {
                    self.event_bus
                        .publish(cuenv_tui::events::TaskEvent::Log {
                            task_name: task_name.clone(),
                            stream: cuenv_tui::events::LogStream::Stdout,
                            content: output.clone(),
                        })
                        .await;
                }
                TaskEvent::TaskError { task_name, error, .. } => {
                    self.event_bus
                        .publish(cuenv_tui::events::TaskEvent::Log {
                            task_name: task_name.clone(),
                            stream: cuenv_tui::events::LogStream::Stderr,
                            content: error.clone(),
                        })
                        .await;
                }
                TaskEvent::TaskSkipped { task_name, .. } => {
                    // For skipped tasks, we treat them as completed for display purposes
                    self.event_bus
                        .publish(cuenv_tui::events::TaskEvent::Completed {
                            task_name: task_name.clone(),
                            exit_code: 0,
                            duration_ms: 0,
                        })
                        .await;
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "tui_formatter"
    }

    fn is_interested(&self, event: &SystemEvent) -> bool {
        // TUI formatter is interested in task events
        matches!(event, SystemEvent::Task(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_tui_formatter_task_started() {
        let formatter = TuiFormatterSubscriber::new();
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
    async fn test_tui_formatter_task_completed() {
        let formatter = TuiFormatterSubscriber::new();
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskCompleted {
                task_name: "test-task".to_string(),
                task_id: "test-id".to_string(),
                duration_ms: 3000,
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
        let formatter = TuiFormatterSubscriber::new();
        assert_eq!(formatter.name(), "tui_formatter");
    }

    #[test]
    fn test_is_interested_in_task_events() {
        let formatter = TuiFormatterSubscriber::new();
        let task_event = SystemEvent::Task(TaskEvent::TaskStarted {
            task_name: "test".to_string(),
            task_id: "test-id".to_string(),
        });
        assert!(formatter.is_interested(&task_event));
    }
}