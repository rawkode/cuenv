//! Task execution and management for cuenv
//!
//! This crate handles task execution, dependency resolution,
//! cross-package references, and command execution.

pub mod builder;
pub mod command_executor;
pub mod cross_package;
pub mod executor;
// pub mod executor_tui;
pub mod registry;
pub mod source;

pub use builder::*;
pub use command_executor::*;
pub use cross_package::*;
pub use executor::*;
// pub use executor_tui::*;
pub use registry::*;
pub use source::*;
