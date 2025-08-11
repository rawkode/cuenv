//! Core domain types for the `cuenv` application.
//!
//! This module provides type-safe wrappers and data structures for various domains
//! within the application. Each sub-module focuses on a specific domain area to
//! maintain clear separation of concerns and improve maintainability.
//!
//! ## Organization
//!
//! - **`capabilities`**: Capability and permission management types
//! - **`commands`**: Command execution and argument handling types  
//! - **`environment`**: Environment variable management types
//! - **`files`**: File path and validation types
//! - **`security`**: Secret handling and security configuration types
//! - **`shared`**: Common types used across multiple domains
//! - **`tasks`**: Task execution pipeline and configuration types

pub mod capabilities;
pub mod commands;
pub mod environment;
pub mod files;
pub mod security;
pub mod shared;
pub mod tasks;

// Re-export all public types for convenient access
pub use capabilities::*;
pub use commands::*;
pub use environment::*;
pub use files::*;
pub use security::*;
pub use shared::*;
pub use tasks::*;
