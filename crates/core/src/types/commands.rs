//! Command-related types for safe command execution

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

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
