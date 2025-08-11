//! Capability-based access control for cache operations
//!
//! This module implements a fine-grained access control system using capability tokens.
//! Each operation requires appropriate capabilities, preventing unauthorized access
//! and enabling secure multi-tenant cache usage.
//!
//! ## Security Model
//!
//! - Capabilities are cryptographically signed tokens
//! - Each token grants specific permissions (read, write, admin)
//! - Tokens include expiration and scope constraints
//! - Zero-trust model: all operations must be authorized

mod authority;
mod authorization;
mod limiting;
mod operations;
mod patterns;
mod tokens;
mod verification;

#[cfg(test)]
mod tests;

// Re-export public types and traits
pub use authority::{CapabilityAuthority, TokenForSigning};
pub use authorization::{AuthorizationResult, CapabilityChecker};
pub use limiting::{RateLimitState, RateLimiter};
pub use operations::CacheOperation;
pub use tokens::{CacheCapability, CapabilityToken, Permission, TokenMetadata};
pub use verification::TokenVerificationResult;

// Re-export for backward compatibility
pub use authority::issue_token;
pub use authorization::check_permission;
pub use patterns::matches_pattern;
pub use verification::verify_token;
