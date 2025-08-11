//! Selective cache key generation with environment variable filtering
//!
//! This module provides intelligent cache key generation that only includes
//! relevant environment variables, similar to Bazel's approach for high cache hit rates.

pub mod config;
pub mod filter;
pub mod generator;
pub mod hash;

// Re-export main types for backward compatibility
pub use config::CacheKeyFilterConfig;
pub use filter::{FilterStats, SmartDefaults};
pub use generator::CacheKeyGenerator;
