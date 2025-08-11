//! Configuration loader for cuenv
//!
//! This module provides a centralized loader that handles all configuration
//! loading operations at startup, including CUE file discovery, parsing,
//! environment resolution, and monorepo detection.

use crate::{
    config::{Config, ConfigBuilder, MonorepoContext, RuntimeOptions},
    CueParser, ParseOptions, ParseResult, SecurityConfig,
};
use cuenv_core::{
    constants::{CUENV_PACKAGE_VAR, DEFAULT_PACKAGE_NAME, ENV_CUE_FILENAME},
    Error, Result,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration loader that handles all startup configuration
pub struct ConfigLoader {
    /// Runtime options to apply
    runtime: RuntimeOptions,
    /// Optional directory to load from (defaults to current directory)
    directory: Option<PathBuf>,
    /// Whether to discover monorepo packages
    discover_monorepo: bool,
}

impl ConfigLoader {
    /// Create a new configuration loader
    pub fn new() -> Self {
        Self {
            runtime: RuntimeOptions::default(),
            directory: None,
            discover_monorepo: true,
        }
    }

    /// Set the directory to load configuration from
    pub fn directory(mut self, dir: PathBuf) -> Self {
        self.directory = Some(dir);
        self
    }

    /// Set runtime options
    pub fn runtime(mut self, runtime: RuntimeOptions) -> Self {
        self.runtime = runtime;
        self
    }

    /// Set the environment to use
    pub fn environment(mut self, env: String) -> Self {
        self.runtime.environment = Some(env);
        self
    }

    /// Set capabilities to enable
    pub fn capabilities(mut self, caps: Vec<String>) -> Self {
        self.runtime.capabilities = caps;
        self
    }

    /// Set cache mode
    pub fn cache_mode(mut self, mode: String) -> Self {
        self.runtime.cache_mode = Some(mode);
        self
    }

    /// Set whether to discover monorepo packages
    pub fn discover_monorepo(mut self, discover: bool) -> Self {
        self.discover_monorepo = discover;
        self
    }

    /// Load the configuration
    pub async fn load(self) -> Result<Config> {
        // Determine working directory
        let working_dir = self
            .directory
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| Error::configuration("Failed to determine working directory"))?;

        // Find env.cue file
        let env_file = self.find_env_file(&working_dir)?;

        // Parse CUE configuration if env file exists
        let parse_result = if let Some(ref env_path) = env_file {
            self.parse_cue_file(env_path)?
        } else {
            // Create empty parse result for directories without env.cue
            ParseResult {
                variables: HashMap::new(),
                metadata: HashMap::new(),
                commands: HashMap::new(),
                tasks: HashMap::new(),
                hooks: HashMap::new(),
            }
        };

        // Extract security configuration from parse result
        let security = self.extract_security_config(&parse_result);

        // Detect monorepo context if enabled
        let monorepo = if self.discover_monorepo {
            self.detect_monorepo_context(&working_dir).await?
        } else {
            None
        };

        // Build the final configuration
        let mut builder = ConfigBuilder::new()
            .working_dir(working_dir)
            .parse_result(parse_result)
            .runtime(self.runtime)
            .security(security);

        if let Some(env_path) = env_file {
            builder = builder.env_file(env_path);
        }

        if let Some(mono_ctx) = monorepo {
            builder = builder.monorepo(mono_ctx);
        }

        builder.build()
    }

    /// Find the env.cue file in the given directory or its parents
    fn find_env_file(&self, start_dir: &Path) -> Result<Option<PathBuf>> {
        let mut current = start_dir.to_path_buf();

        loop {
            let env_file = current.join(ENV_CUE_FILENAME);
            if env_file.exists() {
                return Ok(Some(env_file));
            }

            // Check for .cuenv.allowed file to determine if we should continue
            let allowed_file = current.join(".cuenv.allowed");
            if !allowed_file.exists() && current != start_dir {
                // Stop searching if we've moved up and there's no .cuenv.allowed
                return Ok(None);
            }

            // Move to parent directory
            if !current.pop() {
                break;
            }
        }

        Ok(None)
    }

    /// Parse a CUE file and return the result
    fn parse_cue_file(&self, env_file: &Path) -> Result<ParseResult> {
        let dir = env_file
            .parent()
            .ok_or_else(|| Error::configuration("Invalid env.cue path"))?;

        // Create parse options with runtime settings
        let mut options = ParseOptions::default();
        if let Some(ref env) = self.runtime.environment {
            options.environment = Some(env.clone());
        }
        options.capabilities = self.runtime.capabilities.clone();

        // Get the package name from environment or use default
        let package_name =
            std::env::var(CUENV_PACKAGE_VAR).unwrap_or_else(|_| DEFAULT_PACKAGE_NAME.to_string());

        // Parse the CUE package
        CueParser::eval_package_with_options(dir, &package_name, &options)
    }

    /// Extract security configuration from parse result
    fn extract_security_config(&self, _parse_result: &ParseResult) -> SecurityConfig {
        // TODO: Extract security configuration from parse result
        // For now, return default configuration
        // Return empty security config for now
        // TODO: Extract from parse result when SecurityConfig implements Default
        SecurityConfig {
            restrict_disk: None,
            restrict_network: None,
            read_only_paths: None,
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: None,
            infer_from_inputs_outputs: None,
        }
    }

    /// Detect monorepo context
    async fn detect_monorepo_context(&self, working_dir: &Path) -> Result<Option<MonorepoContext>> {
        // Look for monorepo markers (.cuenv.monorepo or cuenv.yaml)
        let mut current = working_dir.to_path_buf();

        loop {
            let monorepo_marker = current.join(".cuenv.monorepo");
            let cuenv_yaml = current.join("cuenv.yaml");

            if monorepo_marker.exists() || cuenv_yaml.exists() {
                // Found monorepo root
                let packages = self.discover_packages(&current).await?;

                // Determine current package
                let current_package = self.find_current_package(working_dir, &packages);

                return Ok(Some(MonorepoContext {
                    root_dir: current.clone(),
                    current_package,
                    packages,
                }));
            }

            if !current.pop() {
                break;
            }
        }

        Ok(None)
    }

    /// Discover packages in a monorepo
    async fn discover_packages(&self, root: &Path) -> Result<HashMap<String, PathBuf>> {
        let mut packages = HashMap::new();

        // Simple discovery: look for env.cue files in subdirectories
        // In a real implementation, this would be more sophisticated
        for entry in
            std::fs::read_dir(root).map_err(|e| Error::file_system(root, "read directory", e))?
        {
            let entry = entry.map_err(|e| Error::file_system(root, "read entry", e))?;
            let path = entry.path();

            if path.is_dir() {
                let env_file = path.join(ENV_CUE_FILENAME);
                if env_file.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        packages.insert(name.to_string(), path);
                    }
                }
            }
        }

        Ok(packages)
    }

    /// Find the current package based on working directory
    fn find_current_package(
        &self,
        working_dir: &Path,
        packages: &HashMap<String, PathBuf>,
    ) -> Option<String> {
        for (name, path) in packages {
            if working_dir.starts_with(path) {
                return Some(name.clone());
            }
        }
        None
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick helper function to load configuration with defaults
pub async fn load_config() -> Result<Config> {
    ConfigLoader::new().load().await
}

/// Load configuration for a specific directory
pub async fn load_config_from(dir: PathBuf) -> Result<Config> {
    ConfigLoader::new().directory(dir).load().await
}

/// Load configuration with specific runtime options
pub async fn load_config_with_runtime(runtime: RuntimeOptions) -> Result<Config> {
    ConfigLoader::new().runtime(runtime).load().await
}
