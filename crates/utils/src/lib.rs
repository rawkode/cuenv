//! Shared utilities and pure functions for cuenv
//!
//! This crate provides common utility functions that are used throughout
//! the cuenv workspace. All functions here are designed to be pure and
//! side-effect free where possible.

pub mod atomic_file;
pub mod cleanup;
pub mod compression;
pub mod directory;
pub mod file_times;
pub mod hooks_status;
pub mod limits;
pub mod memory;
pub mod network;
pub mod paths;
pub mod resilience;
pub mod sync;
pub mod tracing;
pub mod xdg;

pub use atomic_file::*;
pub use cleanup::*;
pub use compression::*;
pub use directory::*;
pub use file_times::*;
pub use hooks_status::*;
pub use limits::*;
pub use memory::*;
// Re-export specific network items to avoid conflicts with resilience module
pub use network::rate_limit::*;
pub use network::retry::{
    retry_async as network_retry_async, retry_blocking as network_retry_blocking,
    RetryConfig as NetworkRetryConfig,
};
pub use paths::*;
pub use resilience::*;
pub use sync::*;
pub use tracing::*;
pub use xdg::*;
