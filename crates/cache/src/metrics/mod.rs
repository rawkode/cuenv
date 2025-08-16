//! Cache metrics collection and tracking
//!
//! This module provides comprehensive metrics for cache operations including
//! hit rates, latencies, storage efficiency, and access patterns.
//!
//! The metrics system is organized into focused modules:
//! - `core`: Basic structures and initialization
//! - `collection`: Recording operations and updating counters  
//! - `calculation`: Computing derived metrics like hit rates and averages
//! - `snapshot`: Point-in-time metric snapshots
//! - `tests`: Comprehensive test suite including property-based tests

pub mod endpoint;

mod calculation;
mod collection;
mod core;
mod snapshot;

#[cfg(test)]
mod tests;

// Re-export public types from modules
pub use core::CacheMetrics;
