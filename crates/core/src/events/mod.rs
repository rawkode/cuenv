//! Event system for inter-crate communication - Phase 3 Enhanced
//!
//! This module provides a comprehensive event system that allows crates to publish
//! events without directly depending on each other. Enhanced with async subscriber
//! pattern, event filtering, and extensible subscriber system.

// pub mod console; // Removed - EventSubscriber pattern no longer used
// pub mod emitter; // Removed - using tracing now
// pub mod global; // Removed - using tracing now
// pub mod json_log; // Disabled - needs refactoring for pure tracing
// pub mod metrics; // Removed - EventSubscriber pattern no longer used
// pub mod subscriber; // Removed - using tracing now
pub mod types;
pub mod utils;

// Compatibility stubs for removed types
#[derive(Debug, Clone)]
pub struct EnhancedEvent {
    pub system_event: SystemEvent,
    pub metadata: std::collections::HashMap<String, String>,
}

// Stub trait for removed EventSubscriber
pub trait EventSubscriber: Send + Sync {
    fn handle_event(
        &self,
        _event: &EnhancedEvent,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async { Ok(()) })
    }
}

// Re-export subscriber implementations - REMOVED
// pub use console::ConsoleSubscriber;
// pub use json_log::JsonLogSubscriber; // Disabled - needs refactoring
// pub use metrics::MetricsSubscriber;

// Re-export core types - REMOVED (using tracing now)
// pub use emitter::{EventBus, EventEmitter};
// pub use global::{
//     emit_global_event, emit_global_event_with_metadata, global_event_bus, global_event_emitter,
//     initialize_global_events, publish_global_event, register_global_subscriber,
// };
// pub use subscriber::{EnhancedEvent, EventSubscriber};
pub use types::{
    CacheEvent, DependencyEvent, EnvEvent, EventSystemError, LogEvent, LogLevel, PipelineEvent,
    SystemEvent, TaskEvent,
};

// Tests removed - EventEmitter/EventBus removed in favor of tracing
