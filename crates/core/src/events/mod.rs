//! Event subscribers for Phase 3 architecture

pub mod console;
pub mod json_log;
pub mod metrics;

pub use console::ConsoleSubscriber;
pub use json_log::JsonLogSubscriber;
pub use metrics::MetricsSubscriber;

// Re-export common subscriber utilities
pub use crate::events::EventSubscriber;