//! Synchronization primitives for `cuenv`.
//!
//! This module contains utilities for managing concurrent access to shared
//! resources, such as environment variables and process locks, ensuring
//! thread safety and preventing race conditions across multiple instances.
//!
//! ## Key Components
//!
//! - **`env`**: Provides thread-safe and process-safe mechanisms for
//!   manipulating environment variables.

pub mod env;

pub use env::SyncEnv;
