//! Fast path optimizations for common cache operations
//!
//! This module provides specialized implementations for hot code paths
//! to minimize latency and maximize throughput.

mod batch;
mod core;
mod inline;
mod specialized;

pub use batch::BatchGet;
pub use core::FastPathCache;
pub use inline::InlineCache;
pub use specialized::{get_bool, get_json, get_string, get_u64, put_bool, put_string};

#[cfg(test)]
mod tests;