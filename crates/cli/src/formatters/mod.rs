//! Event subscriber formatters for task output
//!
//! This module provides formatters that implement the EventSubscriber trait
//! to handle task execution events and format them for different output modes.

pub mod simple;
pub mod spinner;
// pub mod tui;  // TODO: Fix TUI integration issues

pub use simple::SimpleFormatterSubscriber;
pub use spinner::SpinnerFormatterSubscriber;
// pub use tui::TuiFormatterSubscriber;