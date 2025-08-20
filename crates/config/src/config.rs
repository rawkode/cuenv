//! Centralized configuration structure for cuenv
//!
//! This module provides an immutable configuration object that holds all
//! configuration data loaded at startup, eliminating the need for components
//! to perform their own file I/O or parsing.

use crate::{
    CommandConfig, ConfigSettings, Hook, ParseResult, SecurityConfig, TaskConfig, VariableMetadata,
};
use cuenv_core::{Error, Result};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Runtime options for the application
#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    /// Selected environment (e.g., dev, staging, production)
    pub environment: Option<String>,
    /// Enabled capabilities
    pub capabilities: Vec<String>,
    /// Cache mode configuration
    pub cache_mode: Option<String>,
    /// Whether caching is enabled
    pub cache_enabled: bool,
    /// Audit mode for security tracking
    pub audit_mode: bool,
    /// Output format for tasks
    pub output_format: Option<String>,
    /// Trace output (Chrome trace generation)
    pub trace_output: Option<bool>,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            environment: None,
            capabilities: Vec::new(),
            cache_mode: None,
            cache_enabled: true,
            audit_mode: false,
            output_format: None,
            trace_output: None,
        }
    }
}

impl RuntimeOptions {
    /// Merge with config settings, with CLI options taking precedence
    pub fn merge_with_config(&mut self, config: &ConfigSettings) {
        // CLI args take precedence, so only use config values if CLI didn't provide them

        if self.environment.is_none() {
            self.environment = config.default_environment.clone();
        }

        if self.capabilities.is_empty() {
            if let Some(caps) = &config.default_capabilities {
                self.capabilities = caps.clone();
            }
        }

        if self.cache_mode.is_none() {
            self.cache_mode = config.cache_mode.clone();
        }

        // For cache_enabled, we cannot reliably determine if CLI explicitly set it
        // since RuntimeOptions.cache_enabled is bool, not Option<bool>
        // To maintain CLI precedence, we skip config merging for this field
        // This is consistent with the principle that CLI args should always take precedence

        // For audit_mode, only use config if CLI didn't set it to true
        if !self.audit_mode {
            if let Some(config_audit) = config.audit_mode {
                self.audit_mode = config_audit;
            }
        }

        if self.output_format.is_none() {
            self.output_format = config.output_format.clone();
        }

        if self.trace_output.is_none() {
            self.trace_output = config.trace_output;
        }
    }
}

/// Monorepo context information
#[derive(Debug, Clone)]
pub struct MonorepoContext {
    /// Root directory of the monorepo
    pub root_dir: PathBuf,
    /// Current package name (if in a package)
    pub current_package: Option<String>,
    /// Available packages in the monorepo
    pub packages: HashMap<String, PathBuf>,
}

/// Centralized, immutable configuration for the entire application
#[derive(Debug, Clone)]
pub struct Config {
    /// Working directory where cuenv was invoked
    pub working_dir: PathBuf,
    /// Path to the env.cue file (if found)
    pub env_file: Option<PathBuf>,
    /// Parsed CUE configuration
    pub parse_result: ParseResult,
    /// Runtime options
    pub runtime: RuntimeOptions,
    /// Security configuration
    pub security: SecurityConfig,
    /// Monorepo context (if applicable)
    pub monorepo: Option<MonorepoContext>,
    /// Original environment variables (before cuenv modifications)
    pub original_env: HashMap<String, String>,
}

impl Config {
    /// Create a new configuration instance
    pub fn new(
        working_dir: PathBuf,
        env_file: Option<PathBuf>,
        parse_result: ParseResult,
        runtime: RuntimeOptions,
    ) -> Self {
        Self {
            working_dir,
            env_file,
            parse_result,
            runtime,
            security: SecurityConfig {
                restrict_disk: None,
                restrict_network: None,
                read_only_paths: None,
                read_write_paths: None,
                deny_paths: None,
                allowed_hosts: None,
                infer_from_inputs_outputs: None,
            },
            monorepo: None,
            original_env: std::env::vars().collect(),
        }
    }

    /// Get environment variables for the selected environment
    pub fn get_env_vars(&self) -> Result<HashMap<String, String>> {
        let vars = self.parse_result.variables.clone();

        // Apply environment-specific overrides if an environment is selected
        if let Some(_env_name) = &self.runtime.environment {
            // This would need to be extracted from the ParseResult
            // For now, we'll just return the base variables
            // TODO: Implement environment overlay logic
        }

        // Apply capability filtering if needed
        if !self.runtime.capabilities.is_empty() {
            // TODO: Implement capability filtering
        }

        Ok(vars)
    }

    /// Get tasks available in the configuration
    pub fn get_tasks(&self) -> &HashMap<String, TaskConfig> {
        &self.parse_result.tasks
    }

    /// Get a specific task by name
    pub fn get_task(&self, name: &str) -> Option<&TaskConfig> {
        self.parse_result.tasks.get(name)
    }

    /// Get task nodes (preserving group structure with execution modes)
    pub fn get_task_nodes(&self) -> &IndexMap<String, crate::TaskNode> {
        &self.parse_result.task_nodes
    }

    /// Get commands available in the configuration
    pub fn get_commands(&self) -> &HashMap<String, CommandConfig> {
        &self.parse_result.commands
    }

    /// Get hooks for a specific type
    pub fn get_hooks(&self, hook_type: &str) -> Vec<&Hook> {
        self.parse_result
            .hooks
            .get(hook_type)
            .map(|hooks| hooks.iter().collect())
            .unwrap_or_default()
    }

    /// Get variable metadata
    pub fn get_metadata(&self, var_name: &str) -> Option<&VariableMetadata> {
        self.parse_result.metadata.get(var_name)
    }

    /// Check if a variable is marked as sensitive
    pub fn is_sensitive(&self, _var_name: &str) -> bool {
        // TODO: Add sensitive field to VariableMetadata
        // For now, return false
        false
    }

    /// Get the list of available environments
    pub fn get_environments(&self) -> Vec<String> {
        // TODO: Extract from ParseResult when environment support is added
        vec![]
    }

    /// Check if running in monorepo mode
    pub fn is_monorepo(&self) -> bool {
        self.monorepo.is_some()
    }

    /// Get the monorepo root directory if in monorepo mode
    pub fn monorepo_root(&self) -> Option<&PathBuf> {
        self.monorepo.as_ref().map(|m| &m.root_dir)
    }

    /// Create an Arc wrapper for thread-safe sharing
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

/// Builder pattern for constructing Config instances
#[derive(Debug)]
pub struct ConfigBuilder {
    working_dir: Option<PathBuf>,
    env_file: Option<PathBuf>,
    parse_result: Option<ParseResult>,
    runtime: RuntimeOptions,
    security: SecurityConfig,
    monorepo: Option<MonorepoContext>,
}

impl ConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            working_dir: None,
            env_file: None,
            parse_result: None,
            runtime: RuntimeOptions::default(),
            security: SecurityConfig {
                restrict_disk: None,
                restrict_network: None,
                read_only_paths: None,
                read_write_paths: None,
                deny_paths: None,
                allowed_hosts: None,
                infer_from_inputs_outputs: None,
            },
            monorepo: None,
        }
    }

    /// Set the working directory
    pub fn working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set the env.cue file path
    pub fn env_file(mut self, path: PathBuf) -> Self {
        self.env_file = Some(path);
        self
    }

    /// Set the parsed CUE result
    pub fn parse_result(mut self, result: ParseResult) -> Self {
        self.parse_result = Some(result);
        self
    }

    /// Set runtime options
    pub fn runtime(mut self, runtime: RuntimeOptions) -> Self {
        self.runtime = runtime;
        self
    }

    /// Set the environment
    pub fn environment(mut self, env: String) -> Self {
        self.runtime.environment = Some(env);
        self
    }

    /// Add capabilities
    pub fn capabilities(mut self, caps: Vec<String>) -> Self {
        self.runtime.capabilities = caps;
        self
    }

    /// Set audit mode
    pub fn audit_mode(mut self, audit: bool) -> Self {
        self.runtime.audit_mode = audit;
        self
    }

    /// Set security configuration
    pub fn security(mut self, security: SecurityConfig) -> Self {
        self.security = security;
        self
    }

    /// Set monorepo context
    pub fn monorepo(mut self, context: MonorepoContext) -> Self {
        self.monorepo = Some(context);
        self
    }

    /// Build the final Config instance
    pub fn build(self) -> Result<Config> {
        let working_dir = self
            .working_dir
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| Error::configuration("Working directory not specified"))?;

        let parse_result = self
            .parse_result
            .ok_or_else(|| Error::configuration("Parse result not provided"))?;

        let mut config = Config::new(working_dir, self.env_file, parse_result, self.runtime);
        config.security = self.security;
        config.monorepo = self.monorepo;

        Ok(config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}
