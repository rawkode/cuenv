//! Event system for inter-crate communication - Phase 3 Enhanced
//!
//! This module provides a comprehensive event system that allows crates to publish
//! events without directly depending on each other. Enhanced with async subscriber
//! pattern, event filtering, and extensible subscriber system.

pub mod console;
pub mod json_log;
pub mod metrics;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error};

// Re-export subscriber implementations
pub use console::ConsoleSubscriber;
pub use json_log::JsonLogSubscriber;
pub use metrics::MetricsSubscriber;

/// Task execution events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskEvent {
    /// A task has started execution
    TaskStarted { task_name: String, task_id: String },
    /// A task has completed successfully
    TaskCompleted {
        task_name: String,
        task_id: String,
        duration_ms: u64,
    },
    /// A task has failed
    TaskFailed {
        task_name: String,
        task_id: String,
        error: String,
    },
    /// Task progress update
    TaskProgress {
        task_name: String,
        task_id: String,
        message: String,
    },
    /// Task output (stdout)
    TaskOutput {
        task_name: String,
        task_id: String,
        output: String,
    },
    /// Task error output (stderr)
    TaskError {
        task_name: String,
        task_id: String,
        error: String,
    },
    /// Task skipped due to cache or conditions
    TaskSkipped {
        task_name: String,
        task_id: String,
        reason: String,
    },
}

/// Pipeline execution events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineEvent {
    /// Pipeline execution started
    PipelineStarted {
        total_tasks: usize,
        total_levels: usize,
    },
    /// A level of tasks started
    LevelStarted { level: usize, tasks_in_level: usize },
    /// A level of tasks completed
    LevelCompleted {
        level: usize,
        successful_tasks: usize,
        failed_tasks: usize,
    },
    /// Pipeline execution completed
    PipelineCompleted {
        total_duration_ms: u64,
        successful_tasks: usize,
        failed_tasks: usize,
    },
}

/// Cache-related events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheEvent {
    /// Cache hit for a task
    CacheHit { key: String },
    /// Cache miss for a task
    CacheMiss { key: String },
    /// Cache entry written
    CacheWrite { key: String, size_bytes: u64 },
    /// Cache entry evicted
    CacheEvict { key: String, reason: String },
}

/// Environment loading events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvEvent {
    /// Environment file loading started
    EnvLoading { path: String },
    /// Environment file loaded successfully
    EnvLoaded { path: String, var_count: usize },
    /// Environment file loading failed
    EnvLoadFailed { path: String, error: String },
    /// Environment variable changed
    EnvVarChanged { key: String, is_secret: bool },
}

/// Dependency resolution events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyEvent {
    /// Dependency resolved successfully
    DependencyResolved {
        task_name: String,
        dependency_name: String,
        package_name: Option<String>,
    },
    /// Dependency resolution failed
    DependencyResolutionFailed {
        task_name: String,
        dependency_name: String,
        error: String,
    },
}

/// Main event enum that encompasses all event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    Task(TaskEvent),
    Pipeline(PipelineEvent),
    Cache(CacheEvent),
    Env(EnvEvent),
    Dependency(DependencyEvent),
}

/// Enhanced event with metadata and correlation tracking
#[derive(Debug, Clone)]
pub struct EnhancedEvent {
    /// The actual event
    pub event: SystemEvent,
    /// Timestamp when the event occurred
    pub timestamp: SystemTime,
    /// Optional correlation ID for tracing related events
    pub correlation_id: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Trait for event subscribers
#[async_trait::async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Handle an event
    async fn handle_event(
        &self,
        event: &EnhancedEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Subscriber name for debugging
    fn name(&self) -> &'static str;

    /// Check if subscriber is interested in this event type
    fn is_interested(&self, event: &SystemEvent) -> bool;
}

/// Event emitter for publishing events
pub struct EventEmitter {
    /// Event channel sender
    sender: broadcast::Sender<EnhancedEvent>,
    /// Registered subscribers
    subscribers: RwLock<Vec<Arc<dyn EventSubscriber>>>,
    /// Event correlation context
    correlation_context: RwLock<HashMap<String, String>>,
}

impl EventEmitter {
    /// Create a new event emitter with specified channel capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            subscribers: RwLock::new(Vec::new()),
            correlation_context: RwLock::new(HashMap::new()),
        }
    }

    /// Add a subscriber
    pub async fn add_subscriber(&self, subscriber: Arc<dyn EventSubscriber>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(subscriber);
        debug!(
            "Event subscriber added: {}",
            subscribers.last().unwrap().name()
        );
    }

    /// Remove a subscriber by name
    pub async fn remove_subscriber(&self, name: &str) -> bool {
        let mut subscribers = self.subscribers.write().await;
        let initial_len = subscribers.len();
        subscribers.retain(|s| s.name() != name);
        let removed = subscribers.len() != initial_len;
        if removed {
            debug!("Event subscriber removed: {}", name);
        }
        removed
    }

    /// Set correlation context for subsequent events
    pub async fn set_correlation_context(&self, context: HashMap<String, String>) {
        let mut correlation_context = self.correlation_context.write().await;
        *correlation_context = context;
    }

    /// Clear correlation context
    pub async fn clear_correlation_context(&self) {
        let mut correlation_context = self.correlation_context.write().await;
        correlation_context.clear();
    }

    /// Emit an event to all interested subscribers
    pub async fn emit(&self, event: SystemEvent) {
        self.emit_with_metadata(event, HashMap::new()).await;
    }

    /// Emit an event with custom metadata
    pub async fn emit_with_metadata(&self, event: SystemEvent, metadata: HashMap<String, String>) {
        let correlation_context = self.correlation_context.read().await;
        let correlation_id = correlation_context.get("correlation_id").cloned();

        let mut combined_metadata = correlation_context.clone();
        combined_metadata.extend(metadata);

        let enhanced_event = EnhancedEvent {
            event,
            timestamp: SystemTime::now(),
            correlation_id,
            metadata: combined_metadata,
        };

        // Send to broadcast channel (for potential future use)
        if let Err(e) = self.sender.send(enhanced_event.clone()) {
            debug!("Failed to send event to broadcast channel: {}", e);
        }

        // Notify subscribers directly with parallel processing
        self.notify_subscribers(&enhanced_event).await;
    }

    /// Notify all interested subscribers in parallel
    async fn notify_subscribers(&self, event: &EnhancedEvent) {
        let subscribers = self.subscribers.read().await;

        // Filter interested subscribers
        let interested_subscribers: Vec<_> = subscribers
            .iter()
            .filter(|subscriber| subscriber.is_interested(&event.event))
            .collect();

        if interested_subscribers.is_empty() {
            return;
        }

        // Process subscribers in parallel using join_all
        let handles: Vec<_> = interested_subscribers
            .iter()
            .map(|subscriber| {
                let subscriber = Arc::clone(subscriber);
                let event = event.clone();
                async move {
                    if let Err(e) = subscriber.handle_event(&event).await {
                        error!(
                            subscriber = subscriber.name(),
                            error = %e,
                            "Event subscriber failed to handle event"
                        );
                    }
                }
            })
            .collect();

        futures::future::join_all(handles).await;

        debug!(
            event_type = std::any::type_name_of_val(&event.event),
            subscribers_notified = interested_subscribers.len(),
            "Event published to subscribers"
        );
    }

    /// Get the number of registered subscribers
    pub async fn subscriber_count(&self) -> usize {
        let subscribers = self.subscribers.read().await;
        subscribers.len()
    }

    /// Create a receiver for the broadcast channel (for custom handling)
    pub fn subscribe(&self) -> broadcast::Receiver<EnhancedEvent> {
        self.sender.subscribe()
    }

    /// Publish method alias for backward compatibility
    pub async fn publish(&self, event: SystemEvent) {
        self.emit(event).await;
    }
}

/// Global event emitter instance
static GLOBAL_EVENT_EMITTER: std::sync::OnceLock<Arc<EventEmitter>> = std::sync::OnceLock::new();

/// Initialize global event system with custom capacity
pub fn initialize_global_events(capacity: usize) -> Result<Arc<EventEmitter>, EventSystemError> {
    let emitter = Arc::new(EventEmitter::new(capacity));

    GLOBAL_EVENT_EMITTER
        .set(emitter.clone())
        .map_err(|_| EventSystemError::AlreadyInitialized)?;

    debug!("Global event system initialized with capacity {}", capacity);
    Ok(emitter)
}

/// Get the global event emitter (initializing with default capacity if needed)
pub fn global_event_emitter() -> Arc<EventEmitter> {
    GLOBAL_EVENT_EMITTER
        .get_or_init(|| {
            debug!("Auto-initializing global event system with default capacity");
            Arc::new(EventEmitter::new(10000))
        })
        .clone()
}

/// Emit a global event
pub async fn emit_global_event(event: SystemEvent) {
    let emitter = global_event_emitter();
    emitter.emit(event).await;
}

/// Emit a global event with metadata
pub async fn emit_global_event_with_metadata(
    event: SystemEvent,
    metadata: HashMap<String, String>,
) {
    let emitter = global_event_emitter();
    emitter.emit_with_metadata(event, metadata).await;
}

/// Backward compatibility aliases
pub fn global_event_bus() -> Arc<EventEmitter> {
    global_event_emitter()
}

pub async fn publish_global_event(event: SystemEvent) {
    emit_global_event(event).await;
}

pub async fn register_global_subscriber(subscriber: Arc<dyn EventSubscriber>) {
    let emitter = global_event_emitter();
    emitter.add_subscriber(subscriber).await;
}

/// EventBus alias for backward compatibility  
pub type EventBus = EventEmitter;

/// Event system errors
#[derive(Debug, thiserror::Error)]
pub enum EventSystemError {
    #[error("Event system already initialized")]
    AlreadyInitialized,
    #[error("Event system not initialized")]
    NotInitialized,
    #[error("Subscriber error: {0}")]
    SubscriberError(String),
}

/// Utility functions for common event patterns
pub mod utils {
    use super::*;

    /// Create a task started event
    pub fn task_started(task_name: &str, task_id: &str) -> SystemEvent {
        SystemEvent::Task(TaskEvent::TaskStarted {
            task_name: task_name.to_string(),
            task_id: task_id.to_string(),
        })
    }

    /// Create a task completed event
    pub fn task_completed(task_name: &str, task_id: &str, duration_ms: u64) -> SystemEvent {
        SystemEvent::Task(TaskEvent::TaskCompleted {
            task_name: task_name.to_string(),
            task_id: task_id.to_string(),
            duration_ms,
        })
    }

    /// Create a task failed event
    pub fn task_failed(task_name: &str, task_id: &str, error: &str) -> SystemEvent {
        SystemEvent::Task(TaskEvent::TaskFailed {
            task_name: task_name.to_string(),
            task_id: task_id.to_string(),
            error: error.to_string(),
        })
    }

    /// Create a cache hit event
    pub fn cache_hit(key: &str) -> SystemEvent {
        SystemEvent::Cache(CacheEvent::CacheHit {
            key: key.to_string(),
        })
    }

    /// Create a cache miss event
    pub fn cache_miss(key: &str) -> SystemEvent {
        SystemEvent::Cache(CacheEvent::CacheMiss {
            key: key.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_emitter_creation() {
        let emitter = EventEmitter::new(100);
        assert_eq!(emitter.subscriber_count().await, 0);
    }

    #[tokio::test]
    async fn test_event_emission_without_subscribers() {
        let emitter = EventEmitter::new(100);
        let event = utils::task_started("test", "test-1");

        // Should not panic with no subscribers
        emitter.emit(event).await;
    }

    #[tokio::test]
    async fn test_correlation_context() {
        let emitter = EventEmitter::new(100);

        let mut context = HashMap::new();
        context.insert("correlation_id".to_string(), "test-123".to_string());
        context.insert("user_id".to_string(), "user-456".to_string());

        emitter.set_correlation_context(context).await;

        // The correlation context would be included in emitted events
        // This is tested implicitly through subscriber tests
    }
}
