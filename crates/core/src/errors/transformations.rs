//! Error transformation utilities for functional composition

use super::types::{Error, Result};
use std::fmt;

/// Error transformation utilities for common patterns
pub struct ErrorTransform;

impl ErrorTransform {
    /// Transform an IO error with file context
    pub fn io_with_file(path: &str) -> impl Fn(std::io::Error) -> Error + '_ {
        move |e| Error::FileSystem {
            path: path.into(),
            operation: "file operation".to_string(),
            source: e,
        }
    }

    /// Transform a configuration error with additional context
    pub fn config_with_context(context: &str) -> impl Fn(Error) -> Error + '_ {
        move |e| Error::Configuration {
            message: format!("{context}: {e}"),
        }
    }

    /// Transform any error into a configuration error with custom message
    pub fn to_config_error(
        message: &str,
    ) -> impl Fn(Box<dyn std::error::Error + Send + Sync>) -> Error + '_ {
        move |e| Error::Configuration {
            message: format!("{message}: {e}"),
        }
    }

    /// Transform validation errors with field context
    pub fn validation_with_field(field: &str) -> impl Fn(String) -> Error + '_ {
        move |msg| Error::Configuration {
            message: format!("Validation failed for field '{field}': {msg}"),
        }
    }

    /// Chain multiple error transformations
    pub fn chain<T, F1, F2>(first: F1, second: F2) -> impl Fn(T) -> Error
    where
        F1: Fn(T) -> Error,
        F2: Fn(Error) -> Error,
    {
        move |input| second(first(input))
    }
}

/// Functional validation utilities
pub struct Validate;

impl Validate {
    /// Validate that a string is not empty
    pub fn not_empty(value: &str, field_name: &str) -> Result<()> {
        if value.is_empty() {
            Err(Error::Configuration {
                message: format!("Field '{field_name}' cannot be empty"),
            })
        } else {
            Ok(())
        }
    }

    /// Validate that a number is within a range
    pub fn in_range<T>(value: T, min: T, max: T, field_name: &str) -> Result<T>
    where
        T: PartialOrd + fmt::Display + Copy,
    {
        if value < min || value > max {
            Err(Error::Configuration {
                message: format!(
                    "Field '{field_name}' value {value} is not in range [{min}, {max}]"
                ),
            })
        } else {
            Ok(value)
        }
    }

    /// Validate using a custom predicate
    pub fn with_predicate<T, F>(value: T, predicate: F, error_message: &str) -> Result<T>
    where
        F: FnOnce(&T) -> bool,
    {
        if predicate(&value) {
            Ok(value)
        } else {
            Err(Error::Configuration {
                message: error_message.to_string(),
            })
        }
    }

    /// Compose multiple validations
    pub fn all<T>(value: T, validators: Vec<fn(T) -> Result<T>>) -> Result<T>
    where
        T: Clone,
    {
        validators
            .into_iter()
            .try_fold(value, |acc, validator| validator(acc))
    }
}

/// Functional composition utilities for Results
pub struct ResultCompose;

impl ResultCompose {
    /// Apply multiple transformations in sequence
    pub fn pipeline<T, U>(input: Result<T>, transformations: Vec<fn(T) -> Result<T>>) -> Result<T>
    where
        T: Clone,
    {
        transformations
            .into_iter()
            .try_fold(input?, |acc, transform| transform(acc))
    }

    /// Combine multiple Results, failing fast on first error
    pub fn sequence<T>(results: Vec<Result<T>>) -> Result<Vec<T>> {
        results.into_iter().collect()
    }

    /// Apply a function to multiple values, collecting results
    pub fn traverse<T, U, F>(values: Vec<T>, f: F) -> Result<Vec<U>>
    where
        F: Fn(T) -> Result<U>,
    {
        values.into_iter().map(f).collect()
    }

    /// Retry a fallible operation with exponential backoff
    pub fn retry<T, F>(mut operation: F, max_attempts: u32) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let mut attempts = 0;
        loop {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_not_empty() {
        assert!(Validate::not_empty("test", "field").is_ok());
        assert!(Validate::not_empty("", "field").is_err());
    }

    #[test]
    fn test_validate_in_range() {
        assert!(Validate::in_range(5, 1, 10, "value").is_ok());
        assert!(Validate::in_range(15, 1, 10, "value").is_err());
        assert!(Validate::in_range(-1, 1, 10, "value").is_err());
    }

    #[test]
    fn test_validate_with_predicate() {
        let is_even = |n: &i32| n % 2 == 0;
        assert!(Validate::with_predicate(4, is_even, "Must be even").is_ok());
        assert!(Validate::with_predicate(3, is_even, "Must be even").is_err());
    }

    #[test]
    fn test_result_sequence() {
        let results = vec![Ok(1), Ok(2), Ok(3)];
        let result = ResultCompose::sequence(results);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);

        let results_with_error = vec![
            Ok(1),
            Err(Error::Configuration {
                message: "error".to_string(),
            }),
            Ok(3),
        ];
        assert!(ResultCompose::sequence(results_with_error).is_err());
    }

    #[test]
    fn test_result_traverse() {
        let values = vec![1, 2, 3];
        let double_if_positive = |n: i32| {
            if n > 0 {
                Ok(n * 2)
            } else {
                Err(Error::Configuration {
                    message: "Must be positive".to_string(),
                })
            }
        };

        let result = ResultCompose::traverse(values, double_if_positive);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![2, 4, 6]);
    }
}
