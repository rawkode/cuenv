//! Cache-related configuration types

use serde::{Deserialize, Serialize};

/// Task-specific cache configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskCacheConfig {
    /// Simple boolean configuration
    Simple(bool),
    /// Advanced configuration with custom settings
    Advanced {
        /// Whether caching is enabled for this task
        enabled: bool,
        /// Custom environment filtering configuration (simplified for now)
        env: Option<serde_json::Value>,
    },
}

impl TaskCacheConfig {
    /// Get whether caching is enabled for this task
    pub fn enabled(&self) -> bool {
        match self {
            TaskCacheConfig::Simple(enabled) => *enabled,
            TaskCacheConfig::Advanced { enabled, .. } => *enabled,
        }
    }

    /// Get the environment filter configuration if specified
    pub fn env_filter(&self) -> Option<&serde_json::Value> {
        match self {
            TaskCacheConfig::Simple(_) => None,
            TaskCacheConfig::Advanced { env, .. } => env.as_ref(),
        }
    }
}

impl Default for TaskCacheConfig {
    fn default() -> Self {
        TaskCacheConfig::Simple(true)
    }
}

impl From<bool> for TaskCacheConfig {
    fn from(value: bool) -> Self {
        TaskCacheConfig::Simple(value)
    }
}

/// Cache environment variable filtering configuration for tasks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEnvConfig {
    /// Patterns to include (allowlist)
    pub include: Option<Vec<String>>,
    /// Patterns to exclude (denylist)
    pub exclude: Option<Vec<String>>,
    /// Whether to use smart defaults for common build tools
    pub use_smart_defaults: Option<bool>,
}
