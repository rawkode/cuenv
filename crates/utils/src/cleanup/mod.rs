//! Resource cleanup and error recovery.
//!
//! This module provides RAII guards and global handlers to ensure that
//! temporary resources (files, directories, processes) are properly cleaned up,
//! even in the case of panics or unexpected shutdowns.
//!
//! ## Key Components
//!
//! - **`handler`**: Contains the core `CleanupRegistry` and RAII guards like
//!   `TempFileGuard` and `ProcessGuard`.

pub mod handler;

pub use handler::init_cleanup_handler;
