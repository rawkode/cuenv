//! Production-ready unified cache implementation
//!
//! This module provides a Google-scale cache implementation with:
//! - Zero-copy architecture using memory-mapped files
//! - 4-level sharding for optimal file system performance
//! - Separate metadata storage for efficient scanning
//! - No `?` operators - explicit error handling only
//! - Lock-free concurrent access patterns
//! - Comprehensive observability and metrics

pub mod internal;
pub mod serialization;

// Private modules
mod builder;
mod cleanup;
mod eviction;
mod operations;
mod paths;
mod streaming;
mod trait_impl;
mod types;

// Re-export the main Cache type
pub use types::Cache;

#[cfg(test)]
mod tests;
