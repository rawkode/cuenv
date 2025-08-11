//! Environment-related types for domain-specific operations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

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
