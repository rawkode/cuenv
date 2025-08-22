//! Production-grade error handling for the cache system
//!
//! This module provides comprehensive error types with recovery strategies
//! and detailed context for debugging and operational monitoring.

// Import from the modular error system
mod conversions;
mod display;
mod recovery;
mod security;
mod types;

// Re-export everything for backward compatibility
pub use security::*;
pub use types::*;
