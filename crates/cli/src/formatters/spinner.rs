//! Spinner output formatter using EventSubscriber pattern
//!
//! This formatter provides Docker Compose-style spinner output for task execution events.

use cuenv_core::events::{EnhancedEvent, EventSubscriber, SystemEvent, TaskEvent};
use cuenv_tui::spinner::SpinnerFormatter;
use cuenv_tui::events::{TaskRegistry, TaskState};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Spinner formatter that outputs task events with animated spinners
pub struct SpinnerFormatterSubscriber {
    formatter: Arc<RwLock<SpinnerFormatter>>,
    task_registry: TaskRegistry,
}

impl SpinnerFormatterSubscriber {
    /// Create a new spinner formatter
    pub fn new() -> Self {
        let task_registry = TaskRegistry::new();
        let formatter = SpinnerFormatter::new(task_registry.clone());
        
        Self {
            formatter: Arc::new(RwLock::new(formatter)),
            task_registry,
        }
    }

    /// Initialize the formatter with task execution plan
    pub async fn initialize(&self, plan: &cuenv_task::TaskExecutionPlan) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Register all tasks in the execution plan
        for (task_name, _task_config) in &plan.tasks {
            self.task_registry
                .register_task(task_name.clone(), vec![])
                .await;
            self.task_registry
                .update_task_state(task_name, TaskState::Queued)
                .await;
        }

        // Initialize the spinner formatter
        let mut formatter = self.formatter.write().await;
        formatter.initialize(plan).await.map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to initialize spinner formatter: {}", e)
            )) as Box<dyn std::error::Error + Send + Sync>
        })?;

        Ok(())
    }

    /// Start the animation ticker
    pub async fn start_ticker(&self) -> tokio::task::JoinHandle<()> {
        let formatter = self.formatter.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                let formatter = formatter.read().await;
                if formatter.tick().await.is_err() {
                    break;
                }
            }
        })
    }

    /// Cleanup the formatter
    pub async fn cleanup(&self) -> std::io::Result<()> {
        let formatter = self.formatter.read().await;
        formatter.cleanup()
    }
}

impl Default for SpinnerFormatterSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventSubscriber for SpinnerFormatterSubscriber {
    async fn handle_event(&self, event: &EnhancedEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let SystemEvent::Task(task_event) = &event.event {
            // Convert core task events to TUI events and forward to the spinner formatter
            let tui_event = match task_event {
                TaskEvent::TaskStarted { task_name, .. } => {
                    // Update task registry state
                    self.task_registry
                        .update_task_state(task_name, TaskState::Running)
                        .await;
                    
                    Some(cuenv_tui::events::TaskEvent::Started {
                        task_name: task_name.clone(),
                        timestamp: std::time::Instant::now(),
                    })
                }
                TaskEvent::TaskCompleted { task_name, duration_ms, .. } => {
                    // Update task registry state
                    self.task_registry
                        .update_task_state(task_name, TaskState::Completed)
                        .await;
                    
                    Some(cuenv_tui::events::TaskEvent::Completed {
                        task_name: task_name.clone(),
                        exit_code: 0,
                        duration_ms: *duration_ms,
                    })
                }
                TaskEvent::TaskFailed { task_name, error, .. } => {
                    // Update task registry state
                    self.task_registry
                        .update_task_state(task_name, TaskState::Failed)
                        .await;
                    
                    Some(cuenv_tui::events::TaskEvent::Failed {
                        task_name: task_name.clone(),
                        error: error.clone(),
                        duration_ms: 0, // Duration not available from core event
                    })
                }
                TaskEvent::TaskProgress { task_name, message, .. } => {
                    // Update progress in task registry
                    self.task_registry
                        .update_progress(task_name, message.clone())
                        .await;
                    
                    Some(cuenv_tui::events::TaskEvent::Progress {
                        task_name: task_name.clone(),
                        message: message.clone(),
                    })
                }
                TaskEvent::TaskOutput { task_name, output, .. } => {
                    // Add log entry to task registry
                    self.task_registry
                        .add_log(task_name, cuenv_tui::events::LogStream::Stdout, output.clone())
                        .await;
                    
                    Some(cuenv_tui::events::TaskEvent::Log {
                        task_name: task_name.clone(),
                        stream: cuenv_tui::events::LogStream::Stdout,
                        content: output.clone(),
                    })
                }
                TaskEvent::TaskError { task_name, error, .. } => {
                    // Add error log to task registry
                    self.task_registry
                        .add_log(task_name, cuenv_tui::events::LogStream::Stderr, error.clone())
                        .await;
                    
                    Some(cuenv_tui::events::TaskEvent::Log {
                        task_name: task_name.clone(),
                        stream: cuenv_tui::events::LogStream::Stderr,
                        content: error.clone(),
                    })
                }
                TaskEvent::TaskSkipped { task_name, .. } => {
                    // Update task registry state
                    self.task_registry
                        .update_task_state(task_name, TaskState::Completed)
                        .await;
                    
                    // For skipped tasks, we treat them as completed for display purposes
                    Some(cuenv_tui::events::TaskEvent::Completed {
                        task_name: task_name.clone(),
                        exit_code: 0,
                        duration_ms: 0,
                    })
                }
            };

            // Send the TUI event to the spinner formatter
            if let Some(tui_event) = tui_event {
                let formatter = self.formatter.read().await;
                if let Err(e) = formatter.handle_event(tui_event).await {
                    eprintln!("Warning: Failed to handle spinner event: {}", e);
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "spinner_formatter"
    }

    fn is_interested(&self, event: &SystemEvent) -> bool {
        // Spinner formatter is interested in task events
        matches!(event, SystemEvent::Task(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_spinner_formatter_task_started() {
        let formatter = SpinnerFormatterSubscriber::new();
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
    async fn test_spinner_formatter_task_completed() {
        let formatter = SpinnerFormatterSubscriber::new();
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskCompleted {
                task_name: "test-task".to_string(),
                task_id: "test-id".to_string(),
                duration_ms: 2000,
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
        let formatter = SpinnerFormatterSubscriber::new();
        assert_eq!(formatter.name(), "spinner_formatter");
    }

    #[test]
    fn test_is_interested_in_task_events() {
        let formatter = SpinnerFormatterSubscriber::new();
        let task_event = SystemEvent::Task(TaskEvent::TaskStarted {
            task_name: "test".to_string(),
            task_id: "test-id".to_string(),
        });
        assert!(formatter.is_interested(&task_event));
    }
}