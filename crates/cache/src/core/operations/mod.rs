//! Cache operations module

pub mod get;
pub mod misc;
pub mod put;
mod remove;
pub(super) mod utils;

// Re-export operations (they're implemented directly on Cache type)
