//! General-purpose utilities and support modules for `cuenv`.
//!
//! This module contains a collection of helper functionalities that are used
//! across different parts of the application but do not belong to a specific
//! domain. Each submodule focuses on a distinct area of concern.
//!
//! ## Key Submodules
//!
//! - **`cleanup`**: RAII guards for resource cleanup.
//! - **`compression`**: Utilities for data compression (e.g., gzip).
//! - **`limits`**: Management of system resource limits.
//! - **`memory`**: Memory management helpers.
//! - **`network`**: Networking utilities like rate limiting and retries.
//! - **`resilience`**: Fault-tolerance patterns like circuit breakers.
//! - **`sync`**: Thread and process synchronization primitives.
//! - **`xdg`**: XDG Base Directory Specification compliance.

pub mod cleanup;
pub mod network;
pub mod resilience;
pub mod sync;
