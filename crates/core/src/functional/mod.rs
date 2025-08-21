//! Functional programming utilities for the cuenv project
//!
//! This module provides utilities for functional programming patterns,
//! including composition, pipelines, and enhanced error handling.

pub mod composition;

// Re-export commonly used traits and utilities
pub use composition::operators::{
    backward_compose, constant, curry, flip, forward_compose, identity, uncurry,
};
pub use composition::{Compose, IteratorExt, OptionExt, Pipe};

/// Prelude module for convenient imports
pub mod prelude {
    pub use super::composition::operators::*;
    pub use super::composition::{Compose, IteratorExt, OptionExt, Pipe};
    pub use crate::{async_pipeline, pipeline, try_pipeline};
}
