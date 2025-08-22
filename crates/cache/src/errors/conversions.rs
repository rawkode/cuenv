//! Error conversion utilities

use super::types::{CacheError, RecoveryHint, SerializationOp};
use std::path::PathBuf;

/// Error conversion utilities
impl From<std::io::Error> for CacheError {
    fn from(error: std::io::Error) -> Self {
        use std::io::ErrorKind;

        let recovery_hint = match error.kind() {
            ErrorKind::PermissionDenied => RecoveryHint::ContactAdmin,
            ErrorKind::NotFound => RecoveryHint::Recreate,
            ErrorKind::WouldBlock | ErrorKind::TimedOut => RecoveryHint::RetryWithBackoff {
                initial_delay_ms: 100,
                max_retries: 3,
                backoff_multiplier: 2.0,
            },
            _ => RecoveryHint::ContactAdmin,
        };

        Self::Io {
            path: PathBuf::from("."),
            operation: "unknown",
            source: error,
            recovery_hint,
        }
    }
}

/// Convert serde_json errors to cache errors
impl From<serde_json::Error> for CacheError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization {
            key: String::new(),
            operation: SerializationOp::Deserialize,
            source: Box::new(error),
            recovery_hint: RecoveryHint::Custom("Check JSON format and data types".to_string()),
        }
    }
}

/// Convert cache errors to core errors
impl From<CacheError> for cuenv_core::Error {
    fn from(error: CacheError) -> Self {
        cuenv_core::Error::Configuration {
            message: error.to_string(),
        }
    }
}
