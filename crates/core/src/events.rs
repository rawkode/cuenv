//! Event system for inter-crate communication
//!
//! This module provides a generic event system that allows crates to publish
//! events without directly depending on each other. The TUI can subscribe to
//! these events and update its display accordingly.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

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
    /// Task was skipped (from cache)
    TaskSkipped {
        task_name: String,
        task_id: String,
        reason: String,
    },
}

/// Environment management events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvEvent {
    /// Environment is being loaded
    EnvLoading { path: String },
    /// Environment loaded successfully
    EnvLoaded { path: String, var_count: usize },
    /// Environment load failed
    EnvLoadFailed { path: String, error: String },
    /// Environment variable changed
    EnvVarChanged { key: String, is_secret: bool },
}

/// Cache events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheEvent {
    /// Cache hit
    CacheHit { key: String },
    /// Cache miss
    CacheMiss { key: String },
    /// Cache write
    CacheWrite { key: String, size_bytes: u64 },
    /// Cache eviction
    CacheEvict { key: String, reason: String },
}

/// All possible events in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    Task(TaskEvent),
    Env(EnvEvent),
    Cache(CacheEvent),
}

/// Event bus for publishing and subscribing to events
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<SystemEvent>,
}

impl EventBus {
    /// Create a new event bus with the specified channel capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: SystemEvent) {
        // Ignore send errors (no receivers)
        let _ = self.sender.send(event);
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.sender.subscribe()
    }

    /// Get the number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Global event bus instance (optional - can be used via Arc<EventBus> instead)
static GLOBAL_EVENT_BUS: std::sync::OnceLock<Arc<EventBus>> = std::sync::OnceLock::new();

/// Get or create the global event bus
pub fn global_event_bus() -> Arc<EventBus> {
    GLOBAL_EVENT_BUS
        .get_or_init(|| Arc::new(EventBus::default()))
        .clone()
}

/// Convenience macro for publishing task events
#[macro_export]
macro_rules! publish_task_event {
    ($event:expr) => {
        if let Some(bus) = $crate::events::GLOBAL_EVENT_BUS.get() {
            bus.publish($crate::events::SystemEvent::Task($event));
        }
    };
}

/// Convenience macro for publishing env events
#[macro_export]
macro_rules! publish_env_event {
    ($event:expr) => {
        if let Some(bus) = $crate::events::GLOBAL_EVENT_BUS.get() {
            bus.publish($crate::events::SystemEvent::Env($event));
        }
    };
}

/// Convenience macro for publishing cache events
#[macro_export]
macro_rules! publish_cache_event {
    ($event:expr) => {
        if let Some(bus) = $crate::events::GLOBAL_EVENT_BUS.get() {
            bus.publish($crate::events::SystemEvent::Cache($event));
        }
    };
}
