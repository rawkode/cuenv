//! Hook management for cuenv
//!
//! This crate handles the execution and management of hooks including:
//! - Hook execution with constraints
//! - Nix integration
//! - Environment setup and teardown

pub mod manager;
pub mod nix_executor;

pub use manager::*;
pub use nix_executor::*;
