//! Core domain types, errors, and constants for the `cuenv` application.
//!
//! This module establishes the foundational data structures and error handling
//! mechanisms used throughout the entire codebase. It aims to provide clear,
//! type-safe, and consistent building blocks.
//!
//! ## Key Components
//!
//! - **`errors`**: Defines the primary `Error` enum and `Result` type alias,
//!   centralizing all possible failure modes for predictable error handling.
//! - **`types`**: Contains domain-specific newtype wrappers and data structures
//!   like `EnvironmentVariables` and `SecretReference` to enforce invariants at
//!   the type level.
//! - **`constants`**: A collection of shared, static constants such as environment
//!   variable names and file paths.

// The `mod` statements declare the sub-modules within the `core` module.
// The `pub` keyword makes them accessible from other parts of the crate that
// use `crate::core`.
pub mod constants;
pub mod errors;
pub mod events;
pub mod types;

// The `pub use` statements re-export the most important items from the sub-modules
// so they can be conveniently accessed directly through `crate::core::*` without
// needing to specify the sub-module name. This creates a clean and stable public API
// for the core domain.
pub use self::{
    constants::*,
    errors::{Error, Result, ResultExt},
    events::{CacheEvent, EnvEvent, EventBus, SystemEvent, TaskEvent},
    types::*,
};
