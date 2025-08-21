//! Terminal User Interface for cuenv
//!
//! This crate provides TUI components and formatters for cuenv including:
//! - Spinner output formatter
//! - Interactive terminal UI
//! - Event handling
//! - Application state management

pub mod app;
pub mod components;
pub mod events;
pub mod fallback;
pub mod formatters;
pub mod spinner;
pub mod terminal;

// Main exports
pub use app::TuiApp;
pub use components::{TaskConfigPane, TaskHierarchy, TaskLogsPane, TracingPane};
pub use events::{TaskEvent, TaskRegistry, TaskState, TuiEvent};
pub use fallback::*;
pub use spinner::SpinnerFormatter;
pub use terminal::TerminalManager;

// Export TUI layer for integration (will be redesigned)
pub use formatters::TuiLayer;
