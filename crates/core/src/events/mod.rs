//! Event system for inter-crate communication - Phase 3 Enhanced
//!
//! This module provides a comprehensive event system that allows crates to publish
//! events without directly depending on each other. Enhanced with async subscriber
//! pattern, event filtering, and extensible subscriber system.

pub mod console;
pub mod emitter;
pub mod global;
pub mod json_log;
pub mod metrics;
pub mod subscriber;
pub mod types;
pub mod utils;

// Re-export subscriber implementations
pub use console::ConsoleSubscriber;
pub use json_log::JsonLogSubscriber;
pub use metrics::MetricsSubscriber;

// Re-export core types
pub use emitter::{EventBus, EventEmitter};
pub use global::{
    emit_global_event, emit_global_event_with_metadata, global_event_bus, global_event_emitter,
    initialize_global_events, publish_global_event, register_global_subscriber,
};
pub use subscriber::{EnhancedEvent, EventSubscriber};
pub use types::{
    CacheEvent, DependencyEvent, EnvEvent, EventSystemError, PipelineEvent, SystemEvent, TaskEvent,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
