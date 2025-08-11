//! Event subscriber traits and enhanced events

use crate::events::types::SystemEvent;
use std::collections::HashMap;
use std::time::SystemTime;

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