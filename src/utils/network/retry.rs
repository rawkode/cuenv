use crate::core::errors::{Error, Result};
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Exponential backoff multiplier
    pub multiplier: f64,
    /// Add jitter to retry delays to prevent thundering herd
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a configuration for fast retries (e.g., file operations)
    pub fn fast() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: true,
        }
    }

    /// Create a configuration for network operations
    pub fn network() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: true,
        }
    }

    /// Create a configuration for command execution
    pub fn command() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(5),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Trait for determining if an error is retryable
pub trait RetryableError {
    /// Check if this error should trigger a retry
    fn is_retryable(&self) -> bool;
}

/// Default implementation for std::io::Error
impl RetryableError for std::io::Error {
    fn is_retryable(&self) -> bool {
        use std::io::ErrorKind;
        matches!(
            self.kind(),
            ErrorKind::Interrupted
                | ErrorKind::WouldBlock
                | ErrorKind::TimedOut
                | ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::BrokenPipe
        )
    }
}

/// Implementation for our custom Error type
impl RetryableError for Error {
    fn is_retryable(&self) -> bool {
        match self {
            // Command execution errors might be transient
            Error::CommandExecution { exit_code, .. } => {
                // Retry if no exit code (process killed) or specific exit codes
                exit_code.is_none() || matches!(exit_code, Some(124) | Some(137))
            }
            // File system errors might be transient
            Error::FileSystem { source, .. } => source.is_retryable(),
            // Network/secret resolution errors are often transient
            Error::SecretResolution { .. } => true,
            // Other errors are not retryable by default
            _ => false,
        }
    }
}

/// Execute an async operation with exponential backoff retry
pub async fn retry_async<F, Fut, T, E>(config: RetryConfig, mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, E>>,
    E: Into<Error> + RetryableError + std::fmt::Display,
{
    let mut attempt = 0;
    let mut delay = config.initial_delay;

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if attempt >= config.max_attempts || !err.is_retryable() {
                    return Err(err.into());
                }

                log::warn!(
                    "Attempt {}/{} failed: {}. Retrying in {:?}",
                    attempt,
                    config.max_attempts,
                    err,
                    delay
                );

                // Apply jitter if configured
                let actual_delay = if config.jitter {
                    let jitter = Duration::from_millis(
                        (delay.as_millis() as f64 * rand::random::<f64>() * 0.3) as u64,
                    );
                    delay + jitter
                } else {
                    delay
                };

                sleep(actual_delay).await;

                // Calculate next delay with exponential backoff
                delay =
                    Duration::from_millis((delay.as_millis() as f64 * config.multiplier) as u64)
                        .min(config.max_delay);
            }
        }
    }
}

/// Execute a blocking operation with exponential backoff retry
pub fn retry_blocking<F, T, E>(config: RetryConfig, mut operation: F) -> Result<T>
where
    F: FnMut() -> std::result::Result<T, E>,
    E: Into<Error> + RetryableError + std::fmt::Display,
{
    let mut attempt = 0;
    let mut delay = config.initial_delay;

    loop {
        attempt += 1;

        match operation() {
            Ok(result) => return Ok(result),
            Err(err) => {
                if attempt >= config.max_attempts || !err.is_retryable() {
                    return Err(err.into());
                }

                log::warn!(
                    "Attempt {}/{} failed: {}. Retrying in {:?}",
                    attempt,
                    config.max_attempts,
                    err,
                    delay
                );

                // Apply jitter if configured
                let actual_delay = if config.jitter {
                    let jitter = Duration::from_millis(
                        (delay.as_millis() as f64 * rand::random::<f64>() * 0.3) as u64,
                    );
                    delay + jitter
                } else {
                    delay
                };

                // Use standard thread sleep for blocking retry
                std::thread::sleep(actual_delay);

                // Calculate next delay with exponential backoff
                delay =
                    Duration::from_millis((delay.as_millis() as f64 * config.multiplier) as u64)
                        .min(config.max_delay);
            }
        }
    }
}

/// Convenience functions for common retry scenarios
pub mod convenience {
    use super::*;

    /// Retry a file system operation with fast retry configuration
    pub async fn retry_fs<F, Fut, T>(operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = std::io::Result<T>>,
    {
        retry_async(RetryConfig::fast(), operation).await
    }

    /// Retry a network operation with network retry configuration
    pub async fn retry_network<F, Fut, T>(operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        retry_async(RetryConfig::network(), operation).await
    }

    /// Retry a command execution with command retry configuration
    pub async fn retry_command<F, Fut, T>(operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        retry_async(RetryConfig::command(), operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let config = RetryConfig::fast();
        let result = retry_async(config, || async { Ok::<_, std::io::Error>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            multiplier: 2.0,
            jitter: false,
        };

        let result = retry_async(config, || {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            async move {
                if count < 2 {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "transient",
                    ))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_max_attempts_exceeded() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_attempts: 2,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            multiplier: 2.0,
            jitter: false,
        };

        let result = retry_async(config, || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            async {
                Err::<i32, _>(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "always fails",
                ))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig::default();

        let result = retry_async(config, || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            async {
                Err::<i32, _>(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "not retryable",
                ))
            }
        })
        .await;

        assert!(result.is_err());
        // Should only try once for non-retryable errors
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_retry_blocking() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            multiplier: 2.0,
            jitter: false,
        };

        let result = retry_blocking(config, || {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "transient",
                ))
            } else {
                Ok(42)
            }
        });

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
