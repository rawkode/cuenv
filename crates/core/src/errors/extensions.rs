//! Extension traits for error handling

use super::types::{Error, Result};

/// Extension trait for adding context to Results
pub trait ResultExt<T> {
    /// Add context to a Result
    fn context(self, message: impl Into<String>) -> Result<T>;

    /// Add context with a lazy message
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn context(self, message: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let base_error = e.into();
            Error::Configuration {
                message: format!("{}: {}", message.into(), base_error),
            }
        })
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let base_error = e.into();
            Error::Configuration {
                message: format!("{}: {}", f(), base_error),
            }
        })
    }
}
