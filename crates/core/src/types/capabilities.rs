//! Capability-related types for security and permissions management

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

/// Type-safe wrapper for capabilities
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities(Vec<String>);

impl Capabilities {
    /// Create new empty capabilities
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create from a vector
    #[must_use]
    pub fn from_vec(caps: Vec<String>) -> Self {
        Self(caps)
    }

    /// Check if a capability is present
    #[must_use]
    pub fn contains(&self, capability: &str) -> bool {
        self.0.iter().any(|c| c == capability)
    }

    /// Add a capability
    pub fn add(&mut self, capability: impl Into<String>) {
        let cap = capability.into();
        if !self.contains(&cap) {
            self.0.push(cap);
        }
    }

    /// Remove a capability
    pub fn remove(&mut self, capability: &str) -> bool {
        if let Some(pos) = self.0.iter().position(|c| c == capability) {
            self.0.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get the number of capabilities
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if there are no capabilities
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert to inner Vec
    #[must_use]
    pub fn into_inner(self) -> Vec<String> {
        self.0
    }
}

impl Deref for Capabilities {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<String>> for Capabilities {
    fn from(caps: Vec<String>) -> Self {
        Self(caps)
    }
}

/// Type-safe wrapper for capability names
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityName(String);

impl CapabilityName {
    /// Create a new capability name
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the name as a string slice
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for CapabilityName {
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<&str> for CapabilityName {
    fn from(name: &str) -> Self {
        Self(name.to_string())
    }
}
