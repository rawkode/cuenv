//! Security-related types for secret handling and access control

use crate::errors::{Error, Result};
use std::collections::HashSet;
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Type-safe wrapper for secret references
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretReference(String);

impl SecretReference {
    const PREFIX: &'static str = "cuenv-resolver://";

    /// Create a new secret reference if it has the correct prefix
    pub fn new(reference: impl Into<String>) -> Result<Self> {
        let ref_str = reference.into();
        if ref_str.starts_with(Self::PREFIX) {
            Ok(Self(ref_str))
        } else {
            Err(Error::configuration(format!(
                "invalid secret reference: must start with '{}'",
                Self::PREFIX
            )))
        }
    }

    /// Create a secret reference without validation (for internal use)
    #[must_use]
    pub fn new_unchecked(reference: impl Into<String>) -> Self {
        Self(reference.into())
    }

    /// Get the full reference string
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract the JSON configuration from the reference
    #[must_use]
    pub fn config_json(&self) -> Option<&str> {
        self.0.strip_prefix(Self::PREFIX)
    }

    /// Check if a string is a valid secret reference
    #[must_use]
    pub fn is_secret_reference(value: &str) -> bool {
        value.starts_with(Self::PREFIX)
    }
}

impl fmt::Display for SecretReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type-safe wrapper for secret values with secure handling
#[derive(Debug, Default, Zeroize, ZeroizeOnDrop)]
pub struct SecretValues(#[zeroize(skip)] HashSet<SecretString>);

/// Secure string type that zeroizes on drop
#[derive(Debug, Clone, PartialEq, Eq, Hash, Zeroize, ZeroizeOnDrop)]
struct SecretString(String);

impl SecretValues {
    /// Create new empty secret values
    #[must_use]
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    /// Insert a secret value
    pub fn insert(&mut self, secret: impl Into<String>) {
        self.0.insert(SecretString(secret.into()));
    }

    /// Check if a value is a secret
    #[must_use]
    pub fn contains(&self, value: &str) -> bool {
        self.0.iter().any(|s| s.0 == value)
    }

    /// Get the number of secrets
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if there are no secrets
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Clear all secrets securely
    pub fn clear(&mut self) {
        // Drain all values to ensure they are zeroized via drop
        let _ = self.0.drain();
    }

    /// Get an iterator over the secrets (be careful with the returned values)
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|s| s.0.as_str())
    }
}

// Drop is automatically handled by ZeroizeOnDrop derive
