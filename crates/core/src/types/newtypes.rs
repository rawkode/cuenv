//! Newtype wrappers for enhanced type safety and functional composition

use crate::errors::{Error, Result, Validate};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// A validated task name that cannot be empty
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskName(String);

impl TaskName {
    /// Create a new TaskName with validation
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        Validate::not_empty(&name, "task_name")?;
        Validate::with_predicate(
            name.clone(),
            |n| {
                n.chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
            },
            "Task name must contain only alphanumeric characters, underscores, hyphens, and dots",
        )?;
        Ok(TaskName(name))
    }

    /// Create an unsafe TaskName without validation (use only when input is already validated)
    pub fn new_unchecked(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to String
    pub fn into_string(self) -> String {
        self.0
    }
}

impl Display for TaskName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for TaskName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for TaskName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::new(s)
    }
}

impl TryFrom<&str> for TaskName {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self> {
        Self::new(s)
    }
}

impl TryFrom<String> for TaskName {
    type Error = Error;

    fn try_from(s: String) -> Result<Self> {
        Self::new(s)
    }
}

/// A validated environment variable name
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EnvVarName(String);

impl EnvVarName {
    /// Create a new EnvVarName with validation
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        Validate::not_empty(&name, "env_var_name")?;
        Validate::with_predicate(
            name.clone(),
            |n| n.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
            "Environment variable name must contain only uppercase ASCII letters, digits, and underscores",
        )?;
        Ok(EnvVarName(name))
    }

    /// Create an unsafe EnvVarName without validation
    pub fn new_unchecked(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for EnvVarName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for EnvVarName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for EnvVarName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::new(s)
    }
}

/// A validated file path with additional type safety
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedPath(PathBuf);

impl ValidatedPath {
    /// Create a new ValidatedPath
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            Err(Error::Configuration {
                message: "Path cannot be empty".to_string(),
            })
        } else {
            Ok(Self(path))
        }
    }

    /// Create from a string path
    pub fn from_str(path: &str) -> Result<Self> {
        Self::new(PathBuf::from(path))
    }

    /// Check if the path exists
    pub fn exists(&self) -> bool {
        self.0.exists()
    }

    /// Check if the path is a file
    pub fn is_file(&self) -> bool {
        self.0.is_file()
    }

    /// Check if the path is a directory
    pub fn is_dir(&self) -> bool {
        self.0.is_dir()
    }

    /// Get the parent directory
    pub fn parent(&self) -> Option<ValidatedPath> {
        self.0.parent().map(|p| ValidatedPath(p.to_path_buf()))
    }

    /// Join with another path component
    pub fn join(&self, component: impl AsRef<Path>) -> ValidatedPath {
        ValidatedPath(self.0.join(component))
    }

    /// Convert to PathBuf
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }

    /// Get as Path reference
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl Display for ValidatedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl Deref for ValidatedPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A port number with validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Port(u16);

impl Port {
    /// Create a new Port with validation
    pub fn new(port: u16) -> Result<Self> {
        Validate::in_range(port, 1, 65535, "port").map(Port)
    }

    /// Get the inner value
    pub fn get(&self) -> u16 {
        self.0
    }
}

impl Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Port {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        s.parse::<u16>()
            .map_err(|e| Error::Configuration {
                message: format!("Invalid port number: {}", e),
            })
            .and_then(Self::new)
    }
}

/// A timeout duration in seconds with validation (newtype version)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutSecondsNewtype(u32);

impl TimeoutSecondsNewtype {
    /// Create a new TimeoutSeconds with validation
    pub fn new(seconds: u32) -> Result<Self> {
        Validate::in_range(seconds, 1, 3600, "timeout_seconds") // 1 second to 1 hour
            .map(TimeoutSecondsNewtype)
    }

    /// Get the inner value
    pub fn get(&self) -> u32 {
        self.0
    }

    /// Convert to std::time::Duration
    pub fn to_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.0 as u64)
    }
}

impl Display for TimeoutSecondsNewtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

impl FromStr for TimeoutSecondsNewtype {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        s.parse::<u32>()
            .map_err(|e| Error::Configuration {
                message: format!("Invalid timeout: {}", e),
            })
            .and_then(Self::new)
    }
}

/// A cache size with validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheSize(usize);

impl CacheSize {
    /// Create a new CacheSize with validation
    pub fn new(size: usize) -> Result<Self> {
        Validate::in_range(size, 1, 1_000_000, "cache_size").map(CacheSize)
    }

    /// Get the inner value
    pub fn get(&self) -> usize {
        self.0
    }
}

impl Display for CacheSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for CacheSize {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        s.parse::<usize>()
            .map_err(|e| Error::Configuration {
                message: format!("Invalid cache size: {}", e),
            })
            .and_then(Self::new)
    }
}

/// Safe newtype operations trait for string-based newtypes
pub trait NewtypeStr {
    /// Get the inner value as a string reference
    fn inner_str(&self) -> &str;

    /// Transform the inner value
    fn map_str<U, F>(self, f: F) -> U
    where
        F: FnOnce(String) -> U;

    /// Apply a validation function to the inner string value
    fn validate_str<F>(self, validator: F) -> Result<Self>
    where
        Self: Sized,
        F: FnOnce(&str) -> Result<()>;
}

impl NewtypeStr for TaskName {
    fn inner_str(&self) -> &str {
        &self.0
    }

    fn map_str<U, F>(self, f: F) -> U
    where
        F: FnOnce(String) -> U,
    {
        f(self.0)
    }

    fn validate_str<F>(self, validator: F) -> Result<Self>
    where
        F: FnOnce(&str) -> Result<()>,
    {
        validator(&self.0)?;
        Ok(self)
    }
}

impl NewtypeStr for EnvVarName {
    fn inner_str(&self) -> &str {
        &self.0
    }

    fn map_str<U, F>(self, f: F) -> U
    where
        F: FnOnce(String) -> U,
    {
        f(self.0)
    }

    fn validate_str<F>(self, validator: F) -> Result<Self>
    where
        F: FnOnce(&str) -> Result<()>,
    {
        validator(&self.0)?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_name_validation() {
        assert!(TaskName::new("valid_task-name.1").is_ok());
        assert!(TaskName::new("").is_err());
        assert!(TaskName::new("invalid task name").is_err()); // Contains space
        assert!(TaskName::new("invalid@task").is_err()); // Contains @
    }

    #[test]
    fn test_env_var_name_validation() {
        assert!(EnvVarName::new("VALID_ENV_VAR").is_ok());
        assert!(EnvVarName::new("").is_err());
        assert!(EnvVarName::new("invalid_env_var").is_err()); // Contains lowercase
        assert!(EnvVarName::new("INVALID-ENV-VAR").is_err()); // Contains hyphen
    }

    #[test]
    fn test_port_validation() {
        assert!(Port::new(8080).is_ok());
        assert!(Port::new(80).is_ok());
        assert!(Port::new(0).is_err());
        assert!(Port::new(u16::MAX).is_ok());

        // Test edge case - can't test 65536 directly due to u16 overflow
        assert!(Validate::in_range(65536u32, 1, 65535, "port").is_err());
    }

    #[test]
    fn test_timeout_seconds_validation() {
        assert!(TimeoutSecondsNewtype::new(30).is_ok());
        assert!(TimeoutSecondsNewtype::new(0).is_err());
        assert!(TimeoutSecondsNewtype::new(3601).is_err()); // Over 1 hour
    }

    #[test]
    fn test_validated_path() {
        let path = ValidatedPath::new("/tmp/test").unwrap();
        let joined = path.join("subdir");
        assert_eq!(joined.as_path(), Path::new("/tmp/test/subdir"));
    }
}
