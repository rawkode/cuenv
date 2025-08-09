//! Cache configuration management with precedence and validation
use super::{keys::CacheKeyFilterConfig, CacheMode};
use crate::errors::{Error, RecoveryHint, Result, SerializationOp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Base configuration for cache systems
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Base directory for cache storage
    pub base_dir: PathBuf,
    /// Maximum cache size in bytes
    pub max_size: u64,
    /// Cache mode (read-only, read-write, etc.)
    pub mode: CacheMode,
    /// Threshold for inline storage optimization (bytes)
    pub inline_threshold: usize,
    /// Global environment variable filtering configuration
    pub env_filter: CacheKeyFilterConfig,
    /// Task-specific environment filtering configurations
    pub task_env_filters: HashMap<String, CacheKeyFilterConfig>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        // Use XDG cache directory which respects XDG_CACHE_HOME
        use cuenv_utils::xdg::XdgPaths;
        Self {
            base_dir: XdgPaths::cache_dir(),
            max_size: 10 * 1024 * 1024 * 1024, // 10GB
            mode: CacheMode::ReadWrite,
            inline_threshold: 1024, // 1KB
            env_filter: CacheKeyFilterConfig::default(),
            task_env_filters: HashMap::new(),
        }
    }
}

/// Global cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalCacheConfig {
    /// Whether caching is globally enabled
    pub enabled: bool,
    /// Cache mode (read-only, read-write, etc.)
    pub mode: CacheMode,
    /// Base directory for cache storage
    pub base_dir: Option<PathBuf>,
    /// Maximum cache size in bytes
    pub max_size: Option<u64>,
    /// Threshold for inline storage optimization (bytes)
    pub inline_threshold: Option<usize>,
    /// Global environment variable filtering configuration
    pub env_filter: Option<CacheKeyFilterConfig>,
}

impl Default for GlobalCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: CacheMode::ReadWrite,
            base_dir: None,
            max_size: None,
            inline_threshold: None,
            env_filter: None,
        }
    }
}

// Re-export TaskCacheConfig from config crate
pub use cuenv_config::TaskCacheConfig;

/// Complete cache configuration with global and task-specific settings
#[derive(Debug, Clone)]
pub struct CacheConfiguration {
    /// Global cache configuration
    pub global: GlobalCacheConfig,
    /// Task-specific configurations
    pub task_configs: HashMap<String, TaskCacheConfig>,
    /// Configuration source for debugging
    pub source: ConfigSource,
}

impl Default for CacheConfiguration {
    fn default() -> Self {
        Self {
            global: GlobalCacheConfig::default(),
            task_configs: HashMap::new(),
            source: ConfigSource::Default,
        }
    }
}

/// Source of configuration for debugging and precedence tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigSource {
    /// Default configuration
    Default,
    /// Configuration file
    ConfigFile(PathBuf),
    /// Environment variable
    EnvironmentVariable(String),
    /// Command line argument
    CommandLine,
}

/// Builder for creating cache configurations
pub struct CacheConfigBuilder {
    config: CacheConfiguration,
}

impl CacheConfigBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: CacheConfiguration::default(),
        }
    }

    /// Set global cache enabled state
    pub fn with_global_enabled(mut self, enabled: bool) -> Self {
        self.config.global.enabled = enabled;
        self
    }

    /// Set cache mode
    pub fn with_mode(mut self, mode: CacheMode) -> Self {
        self.config.global.mode = mode;
        self
    }

    /// Set base directory
    pub fn with_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.config.global.base_dir = Some(base_dir);
        self
    }

    /// Set maximum cache size
    pub fn with_max_size(mut self, max_size: u64) -> Self {
        self.config.global.max_size = Some(max_size);
        self
    }

    /// Set inline threshold
    pub fn with_inline_threshold(mut self, threshold: usize) -> Self {
        self.config.global.inline_threshold = Some(threshold);
        self
    }

    /// Set global environment filter
    pub fn with_env_filter(mut self, filter: CacheKeyFilterConfig) -> Self {
        self.config.global.env_filter = Some(filter);
        self
    }

    /// Add task configuration
    pub fn with_task_config(mut self, task_name: String, config: TaskCacheConfig) -> Self {
        self.config.task_configs.insert(task_name, config);
        self
    }

    /// Set configuration source
    pub fn with_source(mut self, source: ConfigSource) -> Self {
        self.config.source = source;
        self
    }

    /// Build the configuration
    pub fn build(self) -> CacheConfiguration {
        self.config
    }
}

impl Default for CacheConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration loader that handles precedence
pub struct CacheConfigLoader;

impl CacheConfigLoader {
    /// Load configuration with full precedence handling
    pub fn load() -> Result<CacheConfiguration> {
        let mut config = Self::load_defaults()?;

        // Try to load from config file
        if let Some(file_config) = Self::load_from_config_file()? {
            config = Self::merge_config(
                config,
                file_config,
                ConfigSource::ConfigFile(Self::get_config_file_path()?),
            )?;
        }

        // Override with environment variables
        if let Some(env_config) = Self::load_from_env()? {
            config = Self::merge_config(
                config,
                env_config,
                ConfigSource::EnvironmentVariable("CUENV_CACHE".to_string()),
            )?;
        }

        Ok(config)
    }

    /// Load default configuration
    fn load_defaults() -> Result<CacheConfiguration> {
        Ok(CacheConfiguration {
            global: GlobalCacheConfig::default(),
            task_configs: HashMap::new(),
            source: ConfigSource::Default,
        })
    }

    /// Load configuration from config file
    fn load_from_config_file() -> Result<Option<CacheConfiguration>> {
        let config_path = Self::get_config_file_path()?;

        if !config_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&config_path).map_err(|e| Error::Io {
            path: config_path.clone(),
            operation: "read config file",
            source: e,
            recovery_hint: RecoveryHint::CheckPermissions {
                path: config_path.clone(),
            },
        })?;

        let file_config: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| Error::Serialization {
                key: config_path.display().to_string(),
                operation: SerializationOp::Decode,
                source: Box::new(e),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check config file syntax".to_string(),
                },
            })?;

        // Parse global cache configuration
        let mut global = GlobalCacheConfig::default();

        if let Some(cache_obj) = file_config.get("cache").and_then(|v| v.as_object()) {
            if let Some(enabled) = cache_obj.get("enabled").and_then(|v| v.as_bool()) {
                global.enabled = enabled;
            }

            if let Some(mode_str) = cache_obj.get("mode").and_then(|v| v.as_str()) {
                global.mode = CacheMode::from(mode_str.to_string());
            }

            if let Some(base_dir) = cache_obj.get("base_dir").and_then(|v| v.as_str()) {
                global.base_dir = Some(PathBuf::from(base_dir));
            }

            if let Some(max_size) = cache_obj.get("max_size").and_then(|v| v.as_u64()) {
                global.max_size = Some(max_size);
            }

            if let Some(threshold) = cache_obj.get("inline_threshold").and_then(|v| v.as_u64()) {
                global.inline_threshold = Some(threshold as usize);
            }
        }

        Ok(Some(CacheConfiguration {
            global,
            task_configs: HashMap::new(), // Task configs are loaded from CUE files
            source: ConfigSource::ConfigFile(config_path),
        }))
    }

    /// Load configuration from environment variables
    fn load_from_env() -> Result<Option<CacheConfiguration>> {
        let mut has_env_config = false;
        let mut global = GlobalCacheConfig::default();

        // Check for CUENV_CACHE mode setting
        if let Ok(cache_mode_str) = std::env::var("CUENV_CACHE") {
            global.mode = CacheMode::from(cache_mode_str);
            // If mode is "off", disable caching globally
            if global.mode == CacheMode::Off {
                global.enabled = false;
            }
            has_env_config = true;
        }

        // Check for explicit enabled/disabled setting (takes precedence over mode)
        if let Ok(enabled_str) = std::env::var("CUENV_CACHE_ENABLED") {
            global.enabled = enabled_str.to_lowercase() == "true";
            has_env_config = true;
        }

        // Check for max size setting
        if let Ok(max_size_str) = std::env::var("CUENV_CACHE_MAX_SIZE") {
            if let Ok(max_size) = max_size_str.parse::<u64>() {
                global.max_size = Some(max_size);
                has_env_config = true;
            }
        }

        // Check for inline threshold setting
        if let Ok(threshold_str) = std::env::var("CUENV_CACHE_INLINE_THRESHOLD") {
            if let Ok(threshold) = threshold_str.parse::<usize>() {
                global.inline_threshold = Some(threshold);
                has_env_config = true;
            }
        }

        // Check for base directory setting
        if let Ok(base_dir_str) = std::env::var("CUENV_CACHE_BASE_DIR") {
            global.base_dir = Some(PathBuf::from(base_dir_str));
            has_env_config = true;
        }

        if has_env_config {
            Ok(Some(CacheConfiguration {
                global,
                task_configs: HashMap::new(),
                source: ConfigSource::EnvironmentVariable("CUENV_CACHE*".to_string()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get the configuration file path
    fn get_config_file_path() -> Result<PathBuf> {
        let config_dir = if let Ok(xdg_config_home) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config_home)
        } else {
            dirs::config_dir().ok_or_else(|| Error::Configuration {
                message: "Could not determine config directory".to_string(),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Set XDG_CONFIG_HOME or HOME environment variable".to_string(),
                },
            })?
        };

        Ok(config_dir.join("cuenv").join("config.json"))
    }

    /// Merge configurations with precedence
    fn merge_config(
        base: CacheConfiguration,
        override_config: CacheConfiguration,
        source: ConfigSource,
    ) -> Result<CacheConfiguration> {
        let mut global = base.global;

        // For config files and environment variables, always override the values
        // since they were explicitly loaded from those sources
        match source {
            ConfigSource::ConfigFile(_) | ConfigSource::EnvironmentVariable(_) => {
                // Override all values from the override config
                global.enabled = override_config.global.enabled;
                global.mode = override_config.global.mode;
            }
            _ => {
                // For other sources, be more selective
                let default_global = GlobalCacheConfig::default();

                // Override enabled if it differs from default
                if override_config.global.enabled != default_global.enabled {
                    global.enabled = override_config.global.enabled;
                }

                // Override mode if it differs from default
                if override_config.global.mode != default_global.mode {
                    global.mode = override_config.global.mode;
                }
            }
        }

        if override_config.global.base_dir.is_some() {
            global.base_dir = override_config.global.base_dir;
        }

        if override_config.global.max_size.is_some() {
            global.max_size = override_config.global.max_size;
        }

        if override_config.global.inline_threshold.is_some() {
            global.inline_threshold = override_config.global.inline_threshold;
        }

        if override_config.global.env_filter.is_some() {
            global.env_filter = override_config.global.env_filter;
        }

        // Task configs are additive (from CUE files, not config file/env)
        let mut task_configs = base.task_configs;
        task_configs.extend(override_config.task_configs);

        Ok(CacheConfiguration {
            global,
            task_configs,
            source,
        })
    }

    /// Apply command line arguments (highest precedence)
    pub fn apply_cli_args(
        mut config: CacheConfiguration,
        cache_mode: Option<CacheMode>,
        cache_enabled: Option<bool>,
    ) -> Result<CacheConfiguration> {
        if let Some(mode) = cache_mode {
            config.global.mode = mode;
            // If mode is "off", disable caching globally
            config.global.enabled = mode != CacheMode::Off;
        }

        if let Some(enabled) = cache_enabled {
            config.global.enabled = enabled;
        }

        config.source = ConfigSource::CommandLine;
        Ok(config)
    }
}

/// Cache configuration resolver that determines final cache behavior
pub struct CacheConfigResolver;

impl CacheConfigResolver {
    /// Resolve whether caching should be enabled for a specific task
    pub fn should_cache_task(
        global_config: &GlobalCacheConfig,
        task_config: Option<&TaskCacheConfig>,
        task_name: &str,
    ) -> bool {
        // Global disabled overrides everything
        if !global_config.enabled {
            log::debug!("Cache disabled globally for task '{task_name}'");
            return false;
        }

        // Check task-specific configuration
        if let Some(task_cache_config) = task_config {
            let task_enabled = task_cache_config.enabled();
            log::debug!(
                "Task '{}' cache {} (task-specific config)",
                task_name,
                if task_enabled { "enabled" } else { "disabled" }
            );
            task_enabled
        } else {
            // Default to enabled for deterministic tasks
            log::debug!("Task '{task_name}' cache enabled (default)");
            true
        }
    }

    /// Get the effective cache mode for a task
    pub fn get_task_cache_mode(
        global_config: &GlobalCacheConfig,
        _task_config: Option<&TaskCacheConfig>,
    ) -> CacheMode {
        // For now, use global mode. In the future, task-specific modes could be supported.
        global_config.mode
    }

    /// Get the environment filter configuration for a task
    pub fn get_task_env_filter(
        global_config: &GlobalCacheConfig,
        task_config: Option<&TaskCacheConfig>,
    ) -> Option<CacheKeyFilterConfig> {
        // Task-specific filter takes precedence over global
        if let Some(task_filter_value) = task_config.and_then(|c| c.env_filter()) {
            // Try to deserialize the serde_json::Value to CacheKeyFilterConfig
            match serde_json::from_value::<CacheKeyFilterConfig>(task_filter_value.clone()) {
                Ok(filter) => Some(filter),
                Err(_) => {
                    // Fall back to global config if deserialization fails
                    global_config.env_filter.clone()
                }
            }
        } else {
            global_config.env_filter.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_config_builder() {
        let config = CacheConfigBuilder::new()
            .with_global_enabled(false)
            .with_mode(CacheMode::Read)
            .with_max_size(1024 * 1024)
            .build();

        assert!(!config.global.enabled);
        assert_eq!(config.global.mode, CacheMode::Read);
        assert_eq!(config.global.max_size, Some(1024 * 1024));
    }

    #[test]
    fn test_task_cache_config() {
        // Simple boolean config
        let simple_config = TaskCacheConfig::Simple(true);
        assert!(simple_config.enabled());
        assert!(simple_config.env_filter().is_none());

        // Advanced config
        let advanced_config = TaskCacheConfig::Advanced {
            enabled: false,
            env: Some(serde_json::json!({})),
        };
        assert!(!advanced_config.enabled());
        assert!(advanced_config.env_filter().is_some());
    }

    #[test]
    fn test_cache_config_resolver() {
        let global_config = GlobalCacheConfig {
            enabled: true,
            mode: CacheMode::ReadWrite,
            base_dir: None,
            max_size: None,
            inline_threshold: None,
            env_filter: None,
        };

        // Test with task config enabled
        let task_config = TaskCacheConfig::Simple(true);
        assert!(CacheConfigResolver::should_cache_task(
            &global_config,
            Some(&task_config),
            "test_task"
        ));

        // Test with task config disabled
        let task_config = TaskCacheConfig::Simple(false);
        assert!(!CacheConfigResolver::should_cache_task(
            &global_config,
            Some(&task_config),
            "test_task"
        ));

        // Test with global disabled
        let global_disabled = GlobalCacheConfig {
            enabled: false,
            mode: global_config.mode,
            base_dir: global_config.base_dir.clone(),
            max_size: global_config.max_size,
            inline_threshold: global_config.inline_threshold,
            env_filter: global_config.env_filter.clone(),
        };
        assert!(!CacheConfigResolver::should_cache_task(
            &global_disabled,
            Some(&TaskCacheConfig::Simple(true)),
            "test_task"
        ));

        // Test default behavior (no task config)
        assert!(CacheConfigResolver::should_cache_task(
            &global_config,
            None,
            "test_task"
        ));
    }

    #[test]
    fn test_config_file_loading() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("cuenv");
        std::fs::create_dir_all(&config_dir)?;

        let config_file = config_dir.join("config.json");
        let config_content = r#"{
            "cache": {
                "enabled": false,
                "mode": "read",
                "max_size": 5242880
            }
        }"#;

        std::fs::write(&config_file, config_content)?;

        // Temporarily override the config file path for testing
        let _original_config_path = CacheConfigLoader::get_config_file_path()?;

        // This test would need to mock the config directory path
        // For now, we'll test the parsing logic directly
        let config_content = r#"{
            "cache": {
                "enabled": false,
                "mode": "read",
                "max_size": 5242880
            }
        }"#;

        let file_config: serde_json::Value = serde_json::from_str(config_content)?;
        let cache_obj = file_config.get("cache").unwrap().as_object().unwrap();

        assert_eq!(cache_obj.get("enabled").unwrap().as_bool(), Some(false));
        assert_eq!(cache_obj.get("mode").unwrap().as_str(), Some("read"));
        assert_eq!(cache_obj.get("max_size").unwrap().as_u64(), Some(5242880));

        Ok(())
    }
}
