//! Cache warming and preloading functionality
//!
//! Provides background cache warming to preload frequently accessed entries.

mod candidates;
mod core;
mod patterns;
mod tracker;
mod types;

pub use core::CacheWarmer;
pub use types::{WarmingConfig, WarmingStats};

#[cfg(test)]
mod tests;
