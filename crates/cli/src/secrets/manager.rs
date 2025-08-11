//! Secret management and resolution coordination
//!
//! This module provides the high-level secret management functionality that coordinates
//! secret resolution across multiple resolvers and handles concurrent resolution of
//! environment variables containing secret references.

use super::resolver::{CommandResolver, SecretResolver};
use crate::core::errors::{Error, Result};
use crate::core::types::{EnvironmentVariables, SecretValues};

/// Container for resolved secrets and their metadata
pub struct ResolvedSecrets {
    /// The environment variables with secret references resolved to actual values
    pub env_vars: EnvironmentVariables,
    /// Set of all resolved secret values for tracking/auditing purposes
    pub secret_values: SecretValues,
}

/// High-level secret manager that coordinates resolution across multiple sources
pub struct SecretManager {
    resolver: Box<dyn SecretResolver>,
}

impl SecretManager {
    /// Create a new SecretManager with default CommandResolver
    pub fn new() -> Self {
        Self {
            // Use up to 10 concurrent secret resolutions
            resolver: Box::new(CommandResolver::new(10)),
        }
    }

    /// Create a SecretManager with a custom resolver
    pub fn with_resolver(resolver: Box<dyn SecretResolver>) -> Self {
        Self { resolver }
    }

    /// Resolve all secret references in the given environment variables
    ///
    /// This method:
    /// 1. Identifies environment variables that contain secret references
    /// 2. Resolves all secrets concurrently for performance
    /// 3. Handles resolution failures gracefully by preserving original values
    /// 4. Returns both the resolved environment and a set of secret values
    ///
    /// # Arguments
    /// * `env_vars` - Environment variables that may contain secret references
    ///
    /// # Returns
    /// * `Ok(ResolvedSecrets)` - Successfully processed all variables
    /// * `Err(Error)` - A critical error occurred during processing
    pub async fn resolve_secrets(&self, env_vars: EnvironmentVariables) -> Result<ResolvedSecrets> {
        let mut resolved_env = EnvironmentVariables::new();
        let mut secret_values = SecretValues::new();

        // Collect all secret resolution tasks
        let mut tasks = Vec::new();

        for (key, value) in env_vars {
            if value.starts_with("cuenv-resolver://") {
                let key_clone = key.clone();
                let value_clone = value.clone();
                let resolver = &self.resolver;

                tasks.push(async move {
                    let result = resolver.resolve(&value_clone).await;
                    match result {
                        Ok(opt) => Ok((key_clone, value_clone, opt)),
                        Err(e) => {
                            tracing::warn!(
                                key = %key_clone,
                                error = %e,
                                "Failed to resolve secret"
                            );
                            // Return Ok with None to indicate failure but preserve the original value
                            Ok((key_clone, value_clone, None))
                        }
                    }
                });
            } else {
                // Non-secret values pass through immediately
                resolved_env.insert(key, value);
            }
        }

        // Resolve all secrets in parallel
        let results: Vec<Result<(String, String, Option<String>)>> =
            futures::future::join_all(tasks).await;

        for result in results {
            let (key, original_value, resolved) = result?;
            if let Some(secret) = resolved {
                resolved_env.insert(key.clone(), secret.clone());
                secret_values.insert(secret);
                tracing::debug!(
                    key = %key,
                    "Resolved secret"
                );
            } else {
                // If resolution failed, keep the original value
                resolved_env.insert(key, original_value);
            }
        }

        Ok(ResolvedSecrets {
            env_vars: resolved_env,
            secret_values,
        })
    }
}

impl Default for SecretManager {
    fn default() -> Self {
        Self::new()
    }
}
