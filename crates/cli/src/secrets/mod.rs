//! Secret resolution and management system
//!
//! This module provides a comprehensive secret resolution system that can handle
//! various secret sources through a pluggable resolver architecture.

mod manager;
mod resolver;

#[cfg(test)]
mod tests;

pub use manager::{ResolvedSecrets, SecretManager};
pub use resolver::{CommandResolver, ResolverConfig, SecretResolver};
