//! Retry logic and recovery suggestions for resilient operations.

use super::config::RetryConfig;
use super::state::CircuitBreaker;
use cuenv_core::{Error, Result};
use std::future::Future;
use tokio::time::sleep;

/// Execute an operation with retry logic
pub async fn retry<F, Fut, T>(config: &RetryConfig, operation: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    log::info!("Operation succeeded after {attempt} retries");
                }
                return Ok(result);
            }
            Err(error) => {
                if attempt < config.max_retries && config.should_retry(&error) {
                    let delay = config.calculate_delay(attempt);
                    log::warn!(
                        "Operation failed (attempt {}/{}), retrying in {:?}: {}",
                        attempt + 1,
                        config.max_retries + 1,
                        delay,
                        error
                    );
                    sleep(delay).await;
                    last_error = Some(error);
                } else {
                    // Don't retry - either max retries reached or error is not retryable
                    return Err(error);
                }
            }
        }
    }

    // This should be unreachable, but just in case
    Err(last_error.unwrap_or_else(|| Error::configuration("Retry loop ended unexpectedly")))
}

/// Retry with circuit breaker protection
pub async fn retry_with_circuit_breaker<F, Fut, T>(
    retry_config: &RetryConfig,
    circuit_breaker: &CircuitBreaker,
    operation: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    retry(retry_config, || circuit_breaker.call(&operation)).await
}

/// Helper to suggest recovery actions based on error type
pub fn suggest_recovery(error: &Error) -> String {
    match error {
        Error::Network { .. } => "Network error: Check your internet connection and try again. \
             If the problem persists, the service may be temporarily unavailable."
            .to_string(),
        Error::FileSystem { .. } => "File system error: Check file permissions and disk space. \
             Ensure the path exists and is accessible."
            .to_string(),
        Error::CommandExecution { .. } => {
            "Command execution failed: Ensure the command is installed and in your PATH. \
             Check the command syntax and arguments."
                .to_string()
        }
        Error::Configuration { message } => {
            format!(
                "Configuration error: {message}. Check your env.cue file for syntax errors \
                 or invalid configurations."
            )
        }
        Error::CueParse { message, .. } => {
            format!("CUE parsing error: {message}. Verify your CUE syntax using 'cue vet'.")
        }
        Error::Environment { message, .. } => {
            format!("Environment error: {message}. Try unloading and reloading the environment.")
        }
        Error::Timeout { .. } => "Operation timed out: The operation took too long to complete. \
             Try again or increase the timeout if possible."
            .to_string(),
        Error::SecretResolution { message, .. } => {
            format!(
                "Secret resolution failed: {message}. Check your secret resolver configuration \
                 and ensure authentication is set up correctly."
            )
        }
        Error::ShellExpansion { message, .. } => {
            format!(
                "Shell expansion failed: {message}. Check the syntax of your shell expressions."
            )
        }
        Error::Json { message, .. } => {
            format!("JSON processing error: {message}. Ensure the data is valid JSON format.")
        }
        Error::Ffi { message, .. } => {
            format!("FFI operation failed: {message}. This is likely an internal error.")
        }
        Error::Unsupported { feature, message } => {
            format!(
                "Unsupported feature '{feature}': {message}. This feature may not be available \
                 on your platform or configuration."
            )
        }
        Error::PermissionDenied { .. } => {
            "Permission denied: Check file permissions and ensure you have the necessary \
             access rights. You may need to run with elevated privileges."
                .to_string()
        }
        _ => "An error occurred. Please check the logs for more details.".to_string(),
    }
}
