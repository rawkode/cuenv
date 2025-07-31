use crate::tui::events::{TaskEvent, TaskRegistry};
use once_cell::sync::OnceCell;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{debug, error};

const EVENT_BUS_CAPACITY: usize = 10000;

// Global event bus instance
static GLOBAL_EVENT_BUS: OnceCell<EventBus> = OnceCell::new();

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<TaskEvent>,
    registry: TaskRegistry,
    subscriber_count: Arc<RwLock<usize>>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_BUS_CAPACITY);
        Self {
            sender,
            registry: TaskRegistry::new(),
            subscriber_count: Arc::new(RwLock::new(0)),
        }
    }

    /// Set the global event bus instance
    pub fn set_global(bus: EventBus) -> Result<(), EventBus> {
        GLOBAL_EVENT_BUS.set(bus)
    }

    /// Get the global event bus instance if set
    pub fn global() -> Option<&'static EventBus> {
        GLOBAL_EVENT_BUS.get()
    }

    pub async fn publish(&self, event: TaskEvent) {
        // Update registry based on event
        match &event {
            TaskEvent::Started { task_name, .. } => {
                self.registry
                    .update_task_state(task_name, crate::tui::events::TaskState::Running)
                    .await;
            }
            TaskEvent::Progress { task_name, message } => {
                self.registry
                    .update_progress(task_name, message.clone())
                    .await;
            }
            TaskEvent::Log {
                task_name,
                stream,
                content,
            } => {
                self.registry
                    .add_log(task_name, stream.clone(), content.clone())
                    .await;
            }
            TaskEvent::Completed {
                task_name,
                exit_code,
                ..
            } => {
                self.registry
                    .update_task_state(task_name, crate::tui::events::TaskState::Completed)
                    .await;
                self.registry.set_exit_code(task_name, *exit_code).await;
            }
            TaskEvent::Failed {
                task_name, error, ..
            } => {
                self.registry
                    .update_task_state(task_name, crate::tui::events::TaskState::Failed)
                    .await;
                self.registry
                    .add_log(
                        task_name,
                        crate::tui::events::LogStream::System,
                        format!("Task failed: {}", error),
                    )
                    .await;
            }
            TaskEvent::Cancelled { task_name } => {
                self.registry
                    .update_task_state(task_name, crate::tui::events::TaskState::Cancelled)
                    .await;
            }
        }

        // Broadcast event to all subscribers
        let receiver_count = self.sender.receiver_count();
        debug!(
            event = ?event,
            receivers = receiver_count,
            "Broadcasting task event"
        );

        if let Err(e) = self.sender.send(event) {
            // No receivers is not an error in our case
            if receiver_count > 0 {
                error!("Failed to broadcast event: {}", e);
            }
        }
    }

    pub fn subscribe(&self) -> EventSubscriber {
        let receiver = self.sender.subscribe();
        EventSubscriber::new(receiver)
    }

    pub async fn register_task(&self, name: String, dependencies: Vec<String>) {
        self.registry.register_task(name, dependencies).await;
    }

    pub fn registry(&self) -> &TaskRegistry {
        &self.registry
    }

    pub async fn subscriber_count(&self) -> usize {
        *self.subscriber_count.read().await
    }

    pub async fn increment_subscribers(&self) {
        let mut count = self.subscriber_count.write().await;
        *count += 1;
    }

    pub async fn decrement_subscribers(&self) {
        let mut count = self.subscriber_count.write().await;
        if *count > 0 {
            *count -= 1;
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventSubscriber {
    stream: BroadcastStream<TaskEvent>,
}

impl EventSubscriber {
    fn new(receiver: broadcast::Receiver<TaskEvent>) -> Self {
        Self {
            stream: BroadcastStream::new(receiver),
        }
    }

    pub async fn recv(&mut self) -> Option<TaskEvent> {
        while let Some(result) = self.stream.next().await {
            match result {
                Ok(event) => return Some(event),
                Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(count)) => {
                    debug!("Event subscriber lagged by {} events", count);
                    continue;
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new();
        let mut subscriber = bus.subscribe();

        let event = TaskEvent::Started {
            task_name: "test_task".to_string(),
            timestamp: Instant::now(),
        };

        bus.publish(event.clone()).await;

        let received = subscriber.recv().await;
        assert!(received.is_some());
        match received.unwrap() {
            TaskEvent::Started { task_name, .. } => {
                assert_eq!(task_name, "test_task");
            }
            _ => panic!("Wrong event type received"),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let mut sub1 = bus.subscribe();
        let mut sub2 = bus.subscribe();

        let event = TaskEvent::Progress {
            task_name: "test_task".to_string(),
            message: "Halfway done".to_string(),
        };

        bus.publish(event.clone()).await;

        // Both subscribers should receive the event
        let recv1 = sub1.recv().await;
        let recv2 = sub2.recv().await;

        assert!(recv1.is_some());
        assert!(recv2.is_some());
    }

    #[tokio::test]
    async fn test_registry_updates() {
        let bus = EventBus::new();

        // Register a task
        bus.register_task("task1".to_string(), vec![]).await;

        // Start the task
        bus.publish(TaskEvent::Started {
            task_name: "task1".to_string(),
            timestamp: Instant::now(),
        })
        .await;

        // Check registry state
        let task = bus.registry().get_task("task1").await.unwrap();
        assert_eq!(task.state, crate::tui::events::TaskState::Running);

        // Complete the task
        bus.publish(TaskEvent::Completed {
            task_name: "task1".to_string(),
            exit_code: 0,
            duration_ms: 1000,
        })
        .await;

        // Check final state
        let task = bus.registry().get_task("task1").await.unwrap();
        assert_eq!(task.state, crate::tui::events::TaskState::Completed);
        assert_eq!(task.exit_code, Some(0));
    }
}
