use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::errors::{Error, Result};

/// Wrapper type for environment variables with domain-specific operations
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentVariables(HashMap<String, String>);

impl EnvironmentVariables {
    /// Create a new empty environment
    #[must_use]
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Create from an existing HashMap
    #[must_use]
    pub fn from_map(map: HashMap<String, String>) -> Self {
        Self(map)
    }

    /// Insert a variable, returning the previous value if any
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.0.insert(key.into(), value.into())
    }

    /// Get a variable by key
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.get(key)
    }

    /// Remove a variable, returning its value if present
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.0.remove(key)
    }

    /// Check if a variable exists
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Merge another set of environment variables into this one
    /// Variables in `other` will overwrite existing ones
    pub fn merge(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    /// Filter variables by a predicate
    #[must_use]
    pub fn filter<F>(&self, predicate: F) -> Self
    where
        F: Fn(&str, &str) -> bool,
    {
        let filtered = self
            .0
            .iter()
            .filter(|(k, v)| predicate(k, v))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self(filtered)
    }

    /// Get the number of variables
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if there are no variables
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get an iterator over the variables
    #[must_use]
    pub fn iter(&self) -> std::collections::hash_map::Iter<String, String> {
        self.0.iter()
    }

    /// Convert to the inner HashMap
    #[must_use]
    pub fn into_inner(self) -> HashMap<String, String> {
        self.0
    }
}

impl Deref for EnvironmentVariables {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EnvironmentVariables {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<HashMap<String, String>> for EnvironmentVariables {
    fn from(map: HashMap<String, String>) -> Self {
        Self(map)
    }
}

impl IntoIterator for EnvironmentVariables {
    type Item = (String, String);
    type IntoIter = std::collections::hash_map::IntoIter<String, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Type-safe wrapper for command arguments
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandArguments(Vec<String>);

impl CommandArguments {
    /// Create new empty arguments
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create from a vector of strings
    #[must_use]
    pub fn from_vec(args: Vec<String>) -> Self {
        Self(args)
    }

    /// Add an argument
    pub fn push(&mut self, arg: impl Into<String>) {
        self.0.push(arg.into());
    }

    /// Add multiple arguments
    pub fn extend<I, S>(&mut self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.0.extend(args.into_iter().map(Into::into));
    }

    /// Get the number of arguments
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if there are no arguments
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert to inner Vec
    #[must_use]
    pub fn into_inner(self) -> Vec<String> {
        self.0
    }

    /// Get a slice of the arguments
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

impl Deref for CommandArguments {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<String>> for CommandArguments {
    fn from(args: Vec<String>) -> Self {
        Self(args)
    }
}

impl IntoIterator for CommandArguments {
    type Item = String;
    type IntoIter = std::vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

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

/// Type-safe wrapper for environment names
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EnvironmentName(String);

impl EnvironmentName {
    /// Create a new environment name
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the name as a string slice
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to inner String
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for EnvironmentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for EnvironmentName {
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<&str> for EnvironmentName {
    fn from(name: &str) -> Self {
        Self(name.to_string())
    }
}

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

/// Type-safe wrapper for command names
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommandName(String);

impl CommandName {
    /// Create a new command name
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the name as a string slice
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to inner String
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for CommandName {
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<&str> for CommandName {
    fn from(name: &str) -> Self {
        Self(name.to_string())
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

/// Type-safe wrapper for CUE file paths
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueFilePath(PathBuf);

impl CueFilePath {
    /// Create a new CUE file path
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if path.extension().and_then(|s| s.to_str()) == Some("cue") {
            Ok(Self(path))
        } else {
            Err(Error::configuration(format!(
                "not a CUE file: {}",
                path.display()
            )))
        }
    }

    /// Create without validation (for internal use)
    #[must_use]
    pub fn new_unchecked(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Get the path
    #[must_use]
    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }

    /// Convert to PathBuf
    #[must_use]
    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

impl fmt::Display for CueFilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl AsRef<std::path::Path> for CueFilePath {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
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

/// Shared string type for immutable strings
pub type SharedString = Arc<str>;
