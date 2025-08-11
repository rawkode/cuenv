//! File-related types for safe path handling

use crate::errors::{Error, Result};
use std::fmt;
use std::path::PathBuf;

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
