//! Cache warming and preloading functionality
//!
//! Provides background cache warming to preload frequently accessed entries.

#![allow(dead_code)]

mod candidates;
mod core;
mod patterns;
mod tracker;
mod types;

pub use core::CacheWarmer;
pub use types::{WarmingConfig, WarmingStats};

#[cfg(test)]
mod tests;