//! Event emitter for publishing and managing events

use crate::events::subscriber::{EnhancedEvent, EventSubscriber};
use crate::events::types::SystemEvent;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error};

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

/// EventBus alias for backward compatibility  
pub type EventBus = EventEmitter;
