//! Environment management for cuenv
//!
//! This crate handles environment variable management, diffing, watching,
//! and caching of environment state.

pub mod cache;
pub mod diff;
pub mod manager;
pub mod source_parser;
pub mod state;
pub mod watcher;

pub use cache::*;
pub use diff::*;
pub use manager::EnvManager;
pub use source_parser::*;
pub use state::StateManager;
pub use watcher::*;
