//! Terminal User Interface for cuenv
//!
//! This crate provides TUI components and formatters for cuenv including:
//! - Spinner output formatter
//! - Interactive terminal UI
//! - Event handling
//! - Application state management

pub mod app;
pub mod components;
// pub mod event_bus; // Removed - using tracing layers now
pub mod events;
pub mod fallback;
pub mod formatters;
pub mod spinner;
pub mod terminal;

pub use app::*;
pub use components::*;
// pub use event_bus::*; // Removed
pub use events::*;
pub use fallback::*;
// Only export SpinnerFormatter from spinner to avoid ambiguity
pub use spinner::SpinnerFormatter;
pub use terminal::*;
