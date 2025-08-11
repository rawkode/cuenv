//! Secret resolver implementations
//!
//! This module provides the core secret resolution functionality through a trait-based
//! architecture, with a command-based resolver implementation that supports retry logic,
//! circuit breakers, and rate limiting.

use crate::audit::{audit_logger, AuditLogger};
use crate::command_executor::{CommandExecutor, CommandExecutorFactory};
use crate::core::errors::{Error, Result};
use crate::core::types::CommandArguments;
use crate::utils::network::rate_limit::RateLimitManager;
use crate::utils::network::retry::{retry_async as retry, RetryConfig};
use crate::utils::resilience::{CircuitBreaker, CircuitBreakerConfig};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

/// Configuration for a command-based secret resolver
#[derive(Debug, Deserialize, Serialize)]
pub struct ResolverConfig {
    pub cmd: String,
    pub args: Vec<String>,
}

/// Trait for resolving secrets from various sources
#[async_trait]
pub trait SecretResolver: Send + Sync {
    /// Resolve a secret reference to its value
    ///
    /// # Arguments
    /// * `reference` - The secret reference to resolve
    ///
    /// # Returns
    /// * `Ok(Some(value))` - Secret was resolved successfully
    /// * `Ok(None)` - Reference is not handled by this resolver
    /// * `Err(error)` - An error occurred during resolution
    async fn resolve(&self, reference: &str) -> Result<Option<String>>;
}

/// Generic command-based secret resolver that uses CUE-defined resolver configurations
pub struct CommandResolver {
    /// Semaphore to limit concurrent secret resolutions
    semaphore: Arc<Semaphore>,
    /// Track if we've shown the initial approval prompt
    approval_shown: Arc<Mutex<bool>>,
    /// Command executor for running external commands
    executor: Box<dyn CommandExecutor>,
    /// Retry configuration for transient failures
    retry_config: RetryConfig,
    /// Circuit breaker for external commands
    circuit_breaker: Arc<CircuitBreaker>,
    /// Rate limiter for secret resolution
    rate_limiter: Option<Arc<RateLimitManager>>,
    /// Audit logger
    audit_logger: Option<Arc<AuditLogger>>,
}

impl CommandResolver {
    /// Create a new CommandResolver with system command executor
    pub fn new(max_concurrent: usize) -> Self {
        Self::with_executor(max_concurrent, CommandExecutorFactory::system())
    }

    /// Create a new CommandResolver with a custom executor
    pub fn with_executor(max_concurrent: usize, executor: Box<dyn CommandExecutor>) -> Self {
        let retry_config = RetryConfig::network();
        let circuit_breaker_config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: std::time::Duration::from_secs(300), // 5 minutes
            break_duration: std::time::Duration::from_secs(60), // 1 minute
            half_open_max_calls: 3,
        };

        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            approval_shown: Arc::new(Mutex::new(false)),
            executor,
            retry_config,
            circuit_breaker: Arc::new(CircuitBreaker::new(circuit_breaker_config)),
            rate_limiter: None,
            audit_logger: audit_logger(),
        }
    }

    /// Set the rate limiter for secret resolution
    #[must_use]
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<RateLimitManager>) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }

    /// Ensure approval is shown once for the session
    async fn ensure_approval(&self) -> Result<()> {
        let mut shown = self.approval_shown.lock().await;
        if !*shown {
            // In a real implementation, we might want to prompt the user here
            // For now, we'll just log that we're starting secret resolution
            tracing::info!("Starting secret resolution. This may prompt for authentication...");
            *shown = true;
        }
        Ok(())
    }

    /// Parse a resolver reference into configuration
    fn parse_resolver_reference(reference: &str) -> Option<ResolverConfig> {
        if let Some(json_str) = reference.strip_prefix("cuenv-resolver://") {
            serde_json::from_str(json_str).ok()
        } else {
            None
        }
    }

    /// Execute a resolver command with retry and circuit breaker
    async fn execute_resolver(&self, config: &ResolverConfig) -> Result<String> {
        // Acquire semaphore permit for rate limiting
        let _permit = match self.semaphore.acquire().await {
            Ok(permit) => permit,
            Err(e) => {
                return Err(Error::configuration(format!(
                    "failed to acquire semaphore for rate limiting: {e}"
                )));
            }
        };

        // Execute with retry and circuit breaker
        let cmd = config.cmd.clone();
        let args = config.args.clone();
        let executor = &self.executor;
        let circuit_breaker = &self.circuit_breaker;

        retry(self.retry_config.clone(), || {
            let cmd = cmd.clone();
            let args = args.clone();
            circuit_breaker.call(|| async move {
                let command_args = CommandArguments::from_vec(args.clone());
                let output = executor.execute(&cmd, &command_args).await.map_err(|e| {
                    Error::command_execution(
                        &cmd,
                        args.clone(),
                        format!("failed to execute command: {e}"),
                        None,
                    )
                })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(Error::command_execution(
                        &cmd,
                        args.clone(),
                        format!("command failed: {stderr}"),
                        output.status.code(),
                    ));
                }

                String::from_utf8(output.stdout)
                    .map(|s| s.trim().to_string())
                    .map_err(|e| {
                        Error::configuration(format!("command output is not valid UTF-8: {e}"))
                    })
            })
        })
        .await
    }
}

#[async_trait]
impl SecretResolver for CommandResolver {
    async fn resolve(&self, reference: &str) -> Result<Option<String>> {
        if let Some(config) = Self::parse_resolver_reference(reference) {
            // Check rate limit if configured
            let _rate_limit_permit = if let Some(ref rate_limiter) = self.rate_limiter {
                match rate_limiter.try_acquire("secrets").await {
                    Ok(Some(permit)) => Some(permit),
                    Ok(None) => None,
                    Err(e) => {
                        // Log rate limit exceeded
                        if let Some(ref logger) = self.audit_logger {
                            let _ = logger.log_rate_limit("secrets", 0, 0, true).await;
                        }
                        return Err(Error::configuration(format!("Rate limit exceeded: {e}")));
                    }
                }
            } else {
                None
            };

            // Ensure we've shown approval message on first resolution
            match self.ensure_approval().await {
                Ok(()) => {}
                Err(e) => return Err(e),
            }

            let result = self.execute_resolver(&config).await;

            // Log the secret resolution attempt
            if let Some(ref logger) = self.audit_logger {
                let _ = logger
                    .log_secret_resolution(
                        &reference[0..20.min(reference.len())], // Truncate for security
                        "command",
                        result.is_ok(),
                        result.as_ref().err().map(|e| e.to_string()),
                    )
                    .await;
            }

            match result {
                Ok(result) => Ok(Some(result)),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        }
    }
}
