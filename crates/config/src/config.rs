//! Centralized configuration management for cuenv
//!
//! This module provides a single, immutable Config struct that serves as the
//! single source of truth for all configuration data, decoupling the rest of
//! the application from the complexities of CUE parsing and environment variable reading.

use crate::{CommandConfig, Hook, ParseOptions, TaskConfig, VariableMetadata};
use cuenv_core::{CacheMode, Error, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Comprehensive, immutable configuration structure that serves as the
/// single source of truth for all configuration data in cuenv.
#[derive(Debug, Clone)]
pub struct Config {
    /// Working directory where configuration was loaded from
    pub working_directory: PathBuf,

    /// Environment variables parsed from CUE configuration
    pub variables: HashMap<String, String>,

    /// Metadata for environment variables (capabilities, etc.)
    pub metadata: HashMap<String, VariableMetadata>,

    /// Available commands with their capability requirements
    pub commands: HashMap<String, CommandConfig>,

    /// Available tasks with their configurations
    pub tasks: HashMap<String, TaskConfig>,

    /// Hooks (onEnter, onExit) defined in the configuration
    pub hooks: HashMap<String, Vec<Hook>>,

    /// CLI-specified options
    pub cli_options: CliOptions,

    /// Parse options used during configuration loading
    pub parse_options: ParseOptions,

    /// Cache configuration
    pub cache: CacheConfig,
}

/// CLI options that affect configuration behavior
#[derive(Debug, Clone, Default)]
pub struct CliOptions {
    /// Cache mode override from CLI
    pub cache_mode: Option<CacheMode>,

    /// Cache enabled/disabled override from CLI
    pub cache_enabled: Option<bool>,

    /// Force flag for operations that can overwrite
    pub force: bool,

    /// Environment name override from CLI
    pub environment: Option<String>,

    /// Capabilities to enable from CLI
    pub capabilities: Vec<String>,

    /// Audit mode flag
    pub audit: bool,

    /// Output format
    pub output_format: String,

    /// Whether to generate trace output
    pub trace_output: bool,
}

/// Cache configuration derived from CLI and environment
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Cache mode (off, read, read-write, write)
    pub mode: CacheMode,

    /// Whether caching is enabled globally
    pub enabled: bool,
    /// Maximum age of cache entries in hours
    pub max_age_hours: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            mode: CacheMode::ReadWrite,
            enabled: true,
            max_age_hours: 168, // 1 week
        }
    }
}

impl Config {
    /// Get a specific environment variable value
    pub fn get_variable(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Get all environment variables
    pub fn get_variables(&self) -> &HashMap<String, String> {
        &self.variables
    }

    /// Get a specific task configuration
    pub fn get_task(&self, name: &str) -> Option<&TaskConfig> {
        self.tasks.get(name)
    }

    /// Get all tasks
    pub fn get_tasks(&self) -> &HashMap<String, TaskConfig> {
        &self.tasks
    }

    /// Get a specific command configuration
    pub fn get_command(&self, name: &str) -> Option<&CommandConfig> {
        self.commands.get(name)
    }

    /// Get hooks of a specific type (e.g., "onEnter", "onExit")
    pub fn get_hooks(&self, hook_type: &str) -> Option<&Vec<Hook>> {
        self.hooks.get(hook_type)
    }

    /// Check if a variable has a specific capability requirement
    pub fn variable_requires_capability(&self, var_name: &str, capability: &str) -> bool {
        if let Some(metadata) = self.metadata.get(var_name) {
            if let Some(var_capability) = &metadata.capability {
                return var_capability == capability;
            }
        }
        false
    }

    /// Get effective environment name (CLI override or parse options)
    pub fn get_environment_name(&self) -> Option<&str> {
        self.cli_options
            .environment
            .as_deref()
            .or(self.parse_options.environment.as_deref())
    }

    /// Get effective capabilities (CLI override or parse options)
    pub fn get_capabilities(&self) -> &[String] {
        if !self.cli_options.capabilities.is_empty() {
            &self.cli_options.capabilities
        } else {
            &self.parse_options.capabilities
        }
    }

    /// Check if a specific capability is enabled
    pub fn has_capability(&self, capability: &str) -> bool {
        self.get_capabilities().contains(&capability.to_string())
    }

    /// Get effective cache mode
    pub fn get_cache_mode(&self) -> CacheMode {
        self.cli_options.cache_mode.unwrap_or(self.cache.mode)
    }
    /// Check if caching is enabled
    pub fn is_cache_enabled(&self) -> bool {
        self.cli_options.cache_enabled.unwrap_or(self.cache.enabled)
    }
}

/// Builder and loader for cuenv configuration
///
/// This struct is responsible for all I/O and parsing operations,
/// including finding/reading env.cue, evaluating CUE, and reading
/// environment variables.
pub struct ConfigLoader {
    /// Working directory to load configuration from
    working_directory: PathBuf,

    /// CLI options to apply to configuration
    cli_options: CliOptions,

    /// Cache configuration
    cache_config: CacheConfig,
}

impl ConfigLoader {
    /// Create a new ConfigLoader for the specified directory
    pub fn new(working_directory: impl Into<PathBuf>) -> Self {
        Self {
            working_directory: working_directory.into(),
            cli_options: CliOptions::default(),
            cache_config: CacheConfig::default(),
        }
    }

    /// Set CLI options
    pub fn with_cli_options(mut self, cli_options: CliOptions) -> Self {
        self.cli_options = cli_options;
        self
    }

    /// Set environment name
    pub fn with_environment(mut self, environment: Option<String>) -> Self {
        // CLI environment takes precedence
        if let Some(env) = environment {
            self.cli_options.environment = Some(env);
        }
        self
    }

    /// Set capabilities
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        // CLI capabilities take precedence
        if !capabilities.is_empty() {
            self.cli_options.capabilities = capabilities;
        }
        self
    }

    /// Set cache configuration
    pub fn with_cache_config(mut self, cache_config: CacheConfig) -> Self {
        self.cache_config = cache_config;
        self
    }

    /// Build parse options from CLI options and environment variables
    fn build_parse_options(&self) -> ParseOptions {
        let mut options = ParseOptions::default();

        // Environment name: CLI takes precedence, then environment variable
        options.environment = self
            .cli_options
            .environment
            .clone()
            .or_else(|| std::env::var(cuenv_core::CUENV_ENV_VAR).ok());

        // Capabilities: CLI takes precedence, then environment variable
        if !self.cli_options.capabilities.is_empty() {
            options.capabilities = self.cli_options.capabilities.clone();
        } else if let Ok(env_caps) = std::env::var(cuenv_core::CUENV_CAPABILITIES_VAR) {
            options.capabilities = env_caps
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        options
    }

    /// Build cache configuration from CLI options and environment variables
    fn build_cache_config(&self) -> CacheConfig {
        let mut config = self.cache_config.clone();

        // Apply CLI overrides
        if let Some(mode) = self.cli_options.cache_mode {
            config.mode = mode;
        }

        if let Some(enabled) = self.cli_options.cache_enabled {
            config.enabled = enabled;
        }

        // Check environment variables for cache configuration
        if let Ok(cache_mode_str) = std::env::var("CUENV_CACHE") {
            match cache_mode_str.as_str() {
                "off" => config.mode = CacheMode::Off,
                "read" => config.mode = CacheMode::Read,
                "read-write" => config.mode = CacheMode::ReadWrite,
                "write" => config.mode = CacheMode::Write,
                _ => {} // Invalid value, keep current
            }
        }

        if let Ok(enabled_str) = std::env::var("CUENV_CACHE_ENABLED") {
            if let Ok(enabled) = enabled_str.parse::<bool>() {
                config.enabled = enabled;
            }
        }

        config
    }

    /// Load and parse the complete configuration
    ///
    /// This method performs all I/O and parsing operations once,
    /// returning an immutable Config object.
    pub fn load(self) -> Result<Config> {
        use crate::CueParser;

        // Build final parse options
        let parse_options = self.build_parse_options();

        // Build final cache configuration
        let cache_config = self.build_cache_config();

        // Check if env.cue exists
        let env_file = self.working_directory.join(cuenv_core::ENV_CUE_FILENAME);
        if !env_file.exists() {
            return Err(Error::configuration(format!(
                "No {} file found in {}",
                cuenv_core::ENV_CUE_FILENAME,
                self.working_directory.display()
            )));
        }

        // Parse CUE configuration
        let parse_result = CueParser::eval_package_with_options(
            &self.working_directory,
            cuenv_core::ENV_PACKAGE_NAME,
            &parse_options,
        )?;

        // Build Config from parse result
        Ok(Config {
            working_directory: self.working_directory,
            variables: parse_result.variables,
            metadata: parse_result.metadata,
            commands: parse_result.commands,
            tasks: parse_result.tasks,
            hooks: parse_result.hooks,
            cli_options: self.cli_options,
            parse_options,
            cache: cache_config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_env(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let cue_dir = temp_dir.path().join("cue.mod");
        fs::create_dir(&cue_dir).unwrap();
        fs::write(cue_dir.join("module.cue"), "module: \"test.com/env\"").unwrap();

        let env_file = temp_dir.path().join("env.cue");
        fs::write(&env_file, content).unwrap();

        temp_dir
    }

    #[test]
    fn test_config_loader_basic() {
        let content = r#"
        package env

        env: {
            DATABASE_URL: "postgres://localhost/test"
            API_KEY: "test-key"
        }

        tasks: {
            test: {
                description: "Run tests"
                command: "npm test"
            }
        }
        "#;

        let temp_dir = create_test_env(content);

        let config = ConfigLoader::new(temp_dir.path()).load().unwrap();

        assert_eq!(
            config.variables.get("DATABASE_URL"),
            Some(&"postgres://localhost/test".to_string())
        );
        assert_eq!(
            config.variables.get("API_KEY"),
            Some(&"test-key".to_string())
        );
        assert!(config.tasks.contains_key("test"));
        assert_eq!(config.working_directory, temp_dir.path());
    }

    #[test]
    fn test_config_loader_with_environment() {
        let content = r#"
        package env

        env: {
            DATABASE_URL: "postgres://localhost/test"

            environment: {
                production: {
                    DATABASE_URL: "postgres://prod.example.com/db"
                }
            }
        }
        "#;

        let temp_dir = create_test_env(content);

        let config = ConfigLoader::new(temp_dir.path())
            .with_environment(Some("production".to_string()))
            .load()
            .unwrap();

        assert_eq!(
            config.variables.get("DATABASE_URL"),
            Some(&"postgres://prod.example.com/db".to_string())
        );
        assert_eq!(config.get_environment_name(), Some("production"));
    }

    #[test]
    fn test_config_loader_missing_file() {
        let temp_dir = TempDir::new().unwrap();

        let result = ConfigLoader::new(temp_dir.path()).load();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No env.cue file found"));
    }

    #[test]
    fn test_config_capabilities() {
        let config = Config {
            working_directory: PathBuf::from("/test"),
            variables: HashMap::new(),
            metadata: HashMap::new(),
            commands: HashMap::new(),
            tasks: HashMap::new(),
            hooks: HashMap::new(),
            cli_options: CliOptions {
                capabilities: vec!["network".to_string(), "filesystem".to_string()],
                ..Default::default()
            },
            parse_options: ParseOptions::default(),
            cache: CacheConfig::default(),
        };

        assert!(config.has_capability("network"));
        assert!(config.has_capability("filesystem"));
        assert!(!config.has_capability("secrets"));
    }
}
