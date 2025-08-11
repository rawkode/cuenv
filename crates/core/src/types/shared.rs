//! Shared types and utilities used across different domains

use std::sync::Arc;

/// Shared string type for immutable strings
pub type SharedString = Arc<str>;
