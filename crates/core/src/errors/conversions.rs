//! Conversion implementations for error types

use super::types::Error;
use std::path::PathBuf;

// Conversion implementations (keeping these as they provide more context than thiserror's #[from])
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::FileSystem {
            path: PathBuf::new(),
            operation: "unknown".to_string(),
            source: error,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Json {
            message: error.to_string(),
            source: error,
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        Error::Configuration {
            message: format!("An internal error occurred: {error}"),
        }
    }
}
