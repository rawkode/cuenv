//! Extension traits for error handling

use super::types::{Error, Result};
use std::fmt::Debug;

/// Extension trait for adding context to Results
pub trait ResultExt<T> {
    /// Add context to a Result
    fn context(self, message: impl Into<String>) -> Result<T>;

    /// Add context with a lazy message
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// Apply a transformation function to the Ok value, chaining Results
    fn and_then_ext<U, F>(self, f: F) -> Result<U>
    where
        F: FnOnce(T) -> Result<U>;

    /// Apply an error transformation function to the Err value
    fn or_else_ext<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(Error) -> Result<T>;

    /// Transform the Ok value using a fallible function
    fn map_result<U, F>(self, f: F) -> Result<U>
    where
        F: FnOnce(T) -> Result<U>;

    /// Transform the error with additional context based on the Ok value type
    fn map_err_with_debug(self) -> Result<T>
    where
        T: Debug;

    /// Provide a fallback value if the Result is an error
    fn or_default(self) -> T
    where
        T: Default;

    /// Apply a function only if the Result is Ok, preserving the original value
    fn tap<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(&T);

    /// Apply a function only if the Result is Err, preserving the original error
    fn tap_err<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(&Error);
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

    fn and_then_ext<U, F>(self, f: F) -> Result<U>
    where
        F: FnOnce(T) -> Result<U>,
    {
        match self {
            Ok(value) => f(value),
            Err(e) => Err(e.into()),
        }
    }

    fn or_else_ext<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(Error) -> Result<T>,
    {
        match self {
            Ok(value) => Ok(value),
            Err(e) => f(e.into()),
        }
    }

    fn map_result<U, F>(self, f: F) -> Result<U>
    where
        F: FnOnce(T) -> Result<U>,
    {
        self.and_then_ext(f)
    }

    fn map_err_with_debug(self) -> Result<T>
    where
        T: Debug,
    {
        self.map_err(|e| {
            let base_error = e.into();
            Error::Configuration {
                message: format!(
                    "Debug context: {:?}, Error: {}",
                    std::any::type_name::<T>(),
                    base_error
                ),
            }
        })
    }

    fn or_default(self) -> T
    where
        T: Default,
    {
        self.unwrap_or_default()
    }

    fn tap<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(&T),
    {
        match self {
            Ok(value) => {
                f(&value);
                Ok(value)
            }
            Err(e) => Err(e.into()),
        }
    }

    fn tap_err<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(&Error),
    {
        match self {
            Ok(value) => Ok(value),
            Err(e) => {
                let error = e.into();
                f(&error);
                Err(error)
            }
        }
    }
}
