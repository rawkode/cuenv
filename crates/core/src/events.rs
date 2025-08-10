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
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, warn};

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

/// Pipeline events for overall execution tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineEvent {
    /// Pipeline execution started
    PipelineStarted {
        total_tasks: usize,
        total_levels: usize,
    },
    /// Pipeline level started
    LevelStarted {
        level: usize,
        tasks_in_level: usize,
    },
    /// Pipeline level completed
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

/// Dependency resolution events (for cross-package dependencies)
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

/// All possible events in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    Task(TaskEvent),
    Env(EnvEvent),
    Cache(CacheEvent),
    Pipeline(PipelineEvent),
    Dependency(DependencyEvent),
}

/// Enhanced event with metadata for Phase 3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedEvent {
    /// The core event
    pub event: SystemEvent,
    /// Event timestamp
    pub timestamp: SystemTime,
    /// Event correlation ID for tracing
    pub correlation_id: Option<String>,
    /// Additional event metadata
    pub metadata: HashMap<String, String>,
}

/// Event subscriber trait for Phase 3 architecture
#[async_trait::async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Handle an event
    async fn handle_event(&self, event: &EnhancedEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Get subscriber name for debugging
    fn name(&self) -> &'static str;
    
    /// Check if this subscriber is interested in the event
    fn is_interested(&self, event: &SystemEvent) -> bool {
        // Default: interested in all events
        let _ = event;
        true
    }
}

/// Enhanced Event Emitter for Phase 3 architecture
#[derive(Clone)]
pub struct EventEmitter {
    /// Broadcast channel for system events
    sender: broadcast::Sender<EnhancedEvent>,
    /// Registered subscribers
    subscribers: Arc<RwLock<Vec<Arc<dyn EventSubscriber>>>>,
    /// Event statistics
    stats: Arc<RwLock<EventStats>>,
    /// Buffering configuration
    buffer_size: usize,
}

/// Event statistics for monitoring
#[derive(Debug, Default)]
pub struct EventStats {
    pub events_published: u64,
    pub events_handled: u64,
    pub events_failed: u64,
    pub subscriber_count: usize,
    pub last_event_time: Option<SystemTime>,
}

impl EventEmitter {
    /// Create a new event emitter
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            subscribers: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(EventStats::default())),
            buffer_size: capacity,
        }
    }

    /// Publish an event to all subscribers
    pub async fn publish(&self, event: SystemEvent) {
        self.publish_with_metadata(event, None, HashMap::new()).await;
    }

    /// Publish an event with additional metadata
    pub async fn publish_with_metadata(
        &self,
        event: SystemEvent,
        correlation_id: Option<String>,
        metadata: HashMap<String, String>,
    ) {
        let enhanced_event = EnhancedEvent {
            event: event.clone(),
            timestamp: SystemTime::now(),
            correlation_id,
            metadata,
        };

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.events_published += 1;
            stats.last_event_time = Some(enhanced_event.timestamp);
        }

        // Broadcast to channel subscribers
        if let Err(e) = self.sender.send(enhanced_event.clone()) {
            // Only log error if there are supposed to be receivers
            if self.sender.receiver_count() > 0 {
                error!("Failed to broadcast event: {}", e);
            }
        }

        // Notify async subscribers
        self.notify_subscribers(&enhanced_event).await;
    }

    /// Register an async subscriber
    pub async fn register_subscriber(&self, subscriber: Arc<dyn EventSubscriber>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(subscriber);

        let mut stats = self.stats.write().await;
        stats.subscriber_count = subscribers.len();
        
        debug!(
            subscriber_name = subscribers.last().unwrap().name(),
            total_subscribers = subscribers.len(),
            "Registered event subscriber"
        );
    }

    /// Unregister a subscriber by name
    pub async fn unregister_subscriber(&self, subscriber_name: &str) {
        let mut subscribers = self.subscribers.write().await;
        let initial_len = subscribers.len();
        
        subscribers.retain(|sub| sub.name() != subscriber_name);
        
        if subscribers.len() < initial_len {
            let mut stats = self.stats.write().await;
            stats.subscriber_count = subscribers.len();
            
            debug!(
                subscriber_name = subscriber_name,
                total_subscribers = subscribers.len(),
                "Unregistered event subscriber"
            );
        }
    }

    /// Subscribe to events via broadcast channel (for backward compatibility)
    pub fn subscribe(&self) -> broadcast::Receiver<EnhancedEvent> {
        self.sender.subscribe()
    }

    /// Get the number of active channel receivers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get current event statistics
    pub async fn stats(&self) -> EventStats {
        self.stats.read().await.clone()
    }

    /// Notify all registered subscribers
    async fn notify_subscribers(&self, event: &EnhancedEvent) {
        let subscribers = self.subscribers.read().await;
        
        for subscriber in subscribers.iter() {
            // Check if subscriber is interested in this event
            if !subscriber.is_interested(&event.event) {
                continue;
            }

            let subscriber_name = subscriber.name();
            
            // Handle the event asynchronously
            match subscriber.handle_event(event).await {
                Ok(()) => {
                    debug!(
                        subscriber = subscriber_name,
                        event_type = std::any::type_name::<SystemEvent>(),
                        "Event handled successfully"
                    );
                    
                    // Update success statistics
                    if let Ok(mut stats) = self.stats.try_write() {
                        stats.events_handled += 1;
                    }
                }
                Err(e) => {
                    error!(
                        subscriber = subscriber_name,
                        error = %e,
                        "Failed to handle event"
                    );
                    
                    // Update error statistics
                    if let Ok(mut stats) = self.stats.try_write() {
                        stats.events_failed += 1;
                    }
                }
            }
        }
    }
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new(10000)
    }
}

/// Legacy EventBus for backward compatibility
#[derive(Clone)]
pub struct EventBus {
    emitter: EventEmitter,
}

impl EventBus {
    /// Create a new event bus with the specified channel capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            emitter: EventEmitter::new(capacity),
        }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: SystemEvent) {
        // Use tokio spawn to handle async from sync context
        let emitter = self.emitter.clone();
        tokio::spawn(async move {
            emitter.publish(event).await;
        });
    }

    /// Subscribe to events (returns the new enhanced events)
    pub fn subscribe(&self) -> broadcast::Receiver<EnhancedEvent> {
        self.emitter.subscribe()
    }

    /// Get the number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.emitter.receiver_count()
    }

    /// Get access to the enhanced emitter
    pub fn emitter(&self) -> &EventEmitter {
        &self.emitter
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Global event emitter instance
static GLOBAL_EVENT_EMITTER: std::sync::OnceLock<Arc<EventEmitter>> = std::sync::OnceLock::new();

/// Global event bus instance (legacy compatibility)
static GLOBAL_EVENT_BUS: std::sync::OnceLock<Arc<EventBus>> = std::sync::OnceLock::new();

/// Get or create the global event emitter (Phase 3)
pub fn global_event_emitter() -> Arc<EventEmitter> {
    GLOBAL_EVENT_EMITTER
        .get_or_init(|| Arc::new(EventEmitter::default()))
        .clone()
}

/// Get or create the global event bus (legacy compatibility)
pub fn global_event_bus() -> Arc<EventBus> {
    GLOBAL_EVENT_BUS
        .get_or_init(|| Arc::new(EventBus::default()))
        .clone()
}

/// Initialize the global event system with custom configuration
pub fn initialize_global_events(capacity: usize) -> Arc<EventEmitter> {
    let emitter = Arc::new(EventEmitter::new(capacity));
    if GLOBAL_EVENT_EMITTER.set(emitter.clone()).is_err() {
        // Already initialized, return the existing one
        return global_event_emitter();
    }
    emitter
}

/// Convenience function to publish events globally
pub async fn publish_global_event(event: SystemEvent) {
    global_event_emitter().publish(event).await;
}

/// Convenience function to register global subscriber
pub async fn register_global_subscriber(subscriber: Arc<dyn EventSubscriber>) {
    global_event_emitter().register_subscriber(subscriber).await;
}

/// Convenience macro for publishing task events (legacy compatibility)
#[macro_export]
macro_rules! publish_task_event {
    ($event:expr) => {
        if let Some(bus) = $crate::events::GLOBAL_EVENT_BUS.get() {
            bus.publish($crate::events::SystemEvent::Task($event));
        }
    };
}

/// Convenience macro for publishing env events (legacy compatibility)
#[macro_export]
macro_rules! publish_env_event {
    ($event:expr) => {
        if let Some(bus) = $crate::events::GLOBAL_EVENT_BUS.get() {
            bus.publish($crate::events::SystemEvent::Env($event));
        }
    };
}

/// Convenience macro for publishing cache events (legacy compatibility)
#[macro_export]
macro_rules! publish_cache_event {
    ($event:expr) => {
        if let Some(bus) = $crate::events::GLOBAL_EVENT_BUS.get() {
            bus.publish($crate::events::SystemEvent::Cache($event));
        }
    };
}

/// Async macro for publishing events with the enhanced emitter
#[macro_export]
macro_rules! publish_event_async {
    ($event:expr) => {
        $crate::events::publish_global_event($event).await;
    };
}

/// Async macro for publishing task events with metadata
#[macro_export]
macro_rules! publish_task_event_async {
    ($event:expr) => {
        $crate::events::publish_global_event($crate::events::SystemEvent::Task($event)).await;
    };
    ($event:expr, $correlation_id:expr, $metadata:expr) => {
        $crate::events::global_event_emitter()
            .publish_with_metadata($crate::events::SystemEvent::Task($event), $correlation_id, $metadata)
            .await;
    };
}

/// Async macro for publishing pipeline events
#[macro_export]
macro_rules! publish_pipeline_event_async {
    ($event:expr) => {
        $crate::events::publish_global_event($crate::events::SystemEvent::Pipeline($event)).await;
    };
}

/// Async macro for publishing dependency events
#[macro_export]
macro_rules! publish_dependency_event_async {
    ($event:expr) => {
        $crate::events::publish_global_event($crate::events::SystemEvent::Dependency($event)).await;
    };
}
