//! Eviction policies for cache memory management
//!
//! Implements LRU, LFU, and ARC eviction strategies with
//! production-grade performance and correctness.

#![allow(dead_code)]

mod factory;
mod policies;
mod traits;

// Re-export public API
pub use factory::create_eviction_policy;
pub use policies::{ArcPolicy, LfuPolicy, LruPolicy};
pub use traits::EvictionPolicy;

#[cfg(test)]
mod tests;
