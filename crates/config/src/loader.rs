//! Configuration loader for centralized configuration management
//!
//! This module provides the `ConfigLoader` that handles all I/O operations
//! for loading configuration data. It centralizes file discovery, CUE evaluation,
//! environment variable reading, and cache integration.

use crate::cache::CueCache;
use crate::config::{Config, PackageInfo, RuntimeSettings, SecurityContext};
use crate::parser::{CueParser, ParseOptions, ParseResult};
use cuenv_core::Result;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

/// Builder for loading configuration in a centralized manner
///
/// The `ConfigLoader` follows the builder pattern and is responsible for all I/O
/// operations related to configuration loading. It handles file discovery, CUE
/// evaluation, environment variable capture, and cache integration.
///
/// # Example
///
/// ```rust,no_run
/// use cuenv_config::{ConfigLoader, RuntimeSettings};
/// use std::path::PathBuf;
///
/// let config = ConfigLoader::new()
///     .with_directory(PathBuf::from("/path/to/project"))
///     .with_environment("dev".to_string())
///     .with_capabilities(vec!["web".to_string(), "db".to_string()])
///     .with_runtime_settings(RuntimeSettings::default())
///     .load()
///     .expect("Failed to load configuration");
/// ```
#[derive(Debug)]
pub struct ConfigLoader {
    /// Directory to search for configuration files
    directory: Option<PathBuf>,

    /// Environment name to load (e.g., "dev", "prod")
    environment_name: Option<String>,

    /// List of capabilities to filter by
    capabilities: Vec<String>,

    /// Runtime settings for the application
    runtime_settings: RuntimeSettings,

    /// Package information for monorepo support
    package_info: Option<PackageInfo>,

    /// Security context for access restrictions
    security_context: SecurityContext,

    /// Whether to run in dry run mode (skip certain operations)
    dry_run: bool,

    /// Custom parse options
    parse_options: Option<ParseOptions>,
}

impl ConfigLoader {
    /// Create a new ConfigLoader with default settings
    pub fn new() -> Self {
        Self {
            directory: None,
            environment_name: None,
            capabilities: Vec::new(),
            runtime_settings: RuntimeSettings::default(),
            package_info: None,
            security_context: SecurityContext::default(),
            dry_run: false,
            parse_options: None,
        }
    }

    /// Set the directory to load configuration from
    pub fn with_directory(mut self, directory: PathBuf) -> Self {
        self.directory = Some(directory);
        self
    }

    /// Set the environment name to load
    pub fn with_environment(mut self, environment_name: String) -> Self {
        self.environment_name = Some(environment_name);
        self
    }

    /// Set the list of capabilities to filter by
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Set the runtime settings
    pub fn with_runtime_settings(mut self, settings: RuntimeSettings) -> Self {
        self.runtime_settings = settings;
        self
    }

    /// Set package information for monorepo support
    pub fn with_package_info(mut self, package_info: PackageInfo) -> Self {
        self.package_info = Some(package_info);
        self
    }

    /// Set security context for access restrictions
    pub fn with_security_context(mut self, security_context: SecurityContext) -> Self {
        self.security_context = security_context;
        self
    }

    /// Enable dry run mode (for CLI completions and discovery)
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Set custom parse options
    pub fn with_parse_options(mut self, parse_options: ParseOptions) -> Self {
        self.parse_options = Some(parse_options);
        self
    }

    /// Load the configuration from the specified directory
    ///
    /// This method performs all I/O operations needed to load configuration:
    /// - Discovers CUE files in the specified directory
    /// - Evaluates CUE configuration using the Go FFI bridge
    /// - Captures current environment variables
    /// - Integrates with cache system for performance
    /// - Resolves secrets if configured
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No directory is specified
    /// - CUE files cannot be found or parsed
    /// - Environment variables cannot be captured
    /// - Cache operations fail
    pub fn load(self) -> Result<Config> {
        let directory = self.directory.clone().ok_or_else(|| {
            cuenv_core::Error::configuration(
                "No directory specified for configuration loading".to_string(),
            )
        })?;

        let environment_name = self
            .environment_name
            .clone()
            .unwrap_or_else(|| "default".to_string());

        // Capture original environment variables
        let original_environment = self.capture_environment_variables()?;

        // Find CUE configuration file
        let cue_file = self.find_cue_file(&directory)?;

        // Load parsed configuration (with caching)
        let parsed_data = self.load_parsed_data(&cue_file)?;

        // Create the final configuration
        let config = Config::with_extensions(
            environment_name,
            self.capabilities,
            directory,
            parsed_data,
            original_environment,
            self.runtime_settings,
            self.package_info,
            self.security_context,
        );

        Ok(config)
    }

    /// Capture current environment variables
    fn capture_environment_variables(&self) -> Result<HashMap<String, String>> {
        let mut env_vars = HashMap::new();

        for (key, value) in env::vars() {
            env_vars.insert(key, value);
        }

        Ok(env_vars)
    }

    /// Find the CUE configuration file in the specified directory
    fn find_cue_file(&self, directory: &Path) -> Result<PathBuf> {
        // Standard CUE file names to look for
        let cue_filenames = ["env.cue", "cuenv.cue", "config.cue"];

        for filename in &cue_filenames {
            let cue_file = directory.join(filename);
            if cue_file.exists() {
                return Ok(cue_file);
            }
        }

        // If in monorepo, also check parent directories
        if let Some(ref package_info) = self.package_info {
            for (_, package_dir) in &package_info.packages {
                for filename in &cue_filenames {
                    let cue_file = package_dir.join(filename);
                    if cue_file.exists() {
                        return Ok(cue_file);
                    }
                }
            }
        }

        Err(cuenv_core::Error::configuration(format!(
            "No CUE configuration file found in directory: {}. Looked for: {}",
            directory.display(),
            cue_filenames.join(", ")
        )))
    }

    /// Load parsed configuration data, using cache if available and valid
    fn load_parsed_data(&self, cue_file: &Path) -> Result<ParseResult> {
        // Try to load from cache first (unless in dry run mode or caching disabled)
        if !self.dry_run && self.runtime_settings.cache_enabled {
            if let Some(cached_result) = CueCache::get(cue_file) {
                return Ok(cached_result);
            }
        }

        // Parse CUE file using the existing parser
        let parse_options = self.parse_options.clone().unwrap_or_default();

        let directory = cue_file
            .parent()
            .ok_or_else(|| cuenv_core::Error::configuration("Invalid CUE file path".to_string()))?;

        let parsed_result = CueParser::eval_package_with_options(directory, "env", &parse_options)?;

        // Cache the result for future use (unless in dry run mode or caching disabled)
        if !self.dry_run && self.runtime_settings.cache_enabled {
            if let Err(e) = CueCache::save(cue_file, &parsed_result) {
                // Log cache write failure but don't fail the entire operation
                tracing::warn!("Failed to write configuration to cache: {}", e);
            }
        }

        Ok(parsed_result)
    }

    /// Discover packages in a monorepo structure
    ///
    /// This method scans the directory tree to find all packages with CUE files
    /// and builds cross-package reference information.
    pub fn discover_packages(directory: &Path) -> Result<PackageInfo> {
        let mut packages = HashMap::new();
        let cross_package_refs = HashMap::new();

        // Use a simple directory traversal to find packages
        // In a real implementation, this could use more sophisticated discovery
        Self::discover_packages_recursive(directory, &mut packages)?;

        let current_package = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("root")
            .to_string();

        Ok(PackageInfo {
            current_package,
            packages,
            cross_package_refs,
        })
    }

    /// Recursively discover packages in a directory tree
    fn discover_packages_recursive(
        directory: &Path,
        packages: &mut HashMap<String, PathBuf>,
    ) -> Result<()> {
        if !directory.is_dir() {
            return Ok(());
        }

        // Check if this directory contains a CUE file
        let cue_filenames = ["env.cue", "cuenv.cue", "config.cue"];
        let has_cue_file = cue_filenames
            .iter()
            .any(|filename| directory.join(filename).exists());

        if has_cue_file {
            let package_name = directory
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();
            packages.insert(package_name, directory.to_path_buf());
        }

        // Recursively check subdirectories
        if let Ok(entries) = std::fs::read_dir(directory) {
            for entry in entries.flatten() {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    Self::discover_packages_recursive(&entry.path(), packages)?;
                }
            }
        }

        Ok(())
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to load configuration from a directory
///
/// This is a shortcut for common use cases where you just need to load
/// configuration from a specific directory with default settings.
///
/// # Example
///
/// ```rust,no_run
/// use cuenv_config::load_config_from_directory;
/// use std::path::PathBuf;
///
/// let config = load_config_from_directory(
///     PathBuf::from("/path/to/project"),
///     "dev".to_string(),
/// ).expect("Failed to load configuration");
/// ```
pub fn load_config_from_directory(directory: PathBuf, environment_name: String) -> Result<Config> {
    ConfigLoader::new()
        .with_directory(directory)
        .with_environment(environment_name)
        .load()
}

/// Convenience function to load configuration with capabilities filtering
///
/// # Example
///
/// ```rust,no_run
/// use cuenv_config::load_config_with_capabilities;
/// use std::path::PathBuf;
///
/// let config = load_config_with_capabilities(
///     PathBuf::from("/path/to/project"),
///     "prod".to_string(),
///     vec!["web".to_string(), "db".to_string()],
/// ).expect("Failed to load configuration");
/// ```
pub fn load_config_with_capabilities(
    directory: PathBuf,
    environment_name: String,
    capabilities: Vec<String>,
) -> Result<Config> {
    ConfigLoader::new()
        .with_directory(directory)
        .with_environment(environment_name)
        .with_capabilities(capabilities)
        .load()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_directory() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let cue_content = r#"
package env

variables: {
    TEST_VAR: "test_value"
}

metadata: {
    TEST_VAR: {
        description: "Test variable"
        sensitive: false
    }
}
"#;
        fs::write(temp_dir.path().join("env.cue"), cue_content).unwrap();
        temp_dir
    }

    #[test]
    fn test_config_loader_builder() {
        let loader = ConfigLoader::new()
            .with_directory(PathBuf::from("/test"))
            .with_environment("dev".to_string())
            .with_capabilities(vec!["web".to_string()])
            .with_dry_run(true);

        assert_eq!(loader.directory, Some(PathBuf::from("/test")));
        assert_eq!(loader.environment_name, Some("dev".to_string()));
        assert_eq!(loader.capabilities, vec!["web".to_string()]);
        assert!(loader.dry_run);
    }

    #[test]
    fn test_find_cue_file() {
        let temp_dir = create_test_directory();
        let loader = ConfigLoader::new();

        let cue_file = loader.find_cue_file(temp_dir.path()).unwrap();
        assert_eq!(cue_file.file_name().unwrap(), "env.cue");
    }

    #[test]
    fn test_find_cue_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::new();

        let result = loader.find_cue_file(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_capture_environment_variables() {
        let loader = ConfigLoader::new();
        let env_vars = loader.capture_environment_variables().unwrap();

        // Should capture at least some environment variables
        assert!(!env_vars.is_empty());

        // Should include PATH on Unix systems
        #[cfg(unix)]
        assert!(env_vars.contains_key("PATH"));
    }

    #[test]
    fn test_discover_packages_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = ConfigLoader::discover_packages(temp_dir.path()).unwrap();

        assert!(result.packages.is_empty());
    }

    #[test]
    fn test_discover_packages_with_cue_file() {
        let temp_dir = create_test_directory();
        let result = ConfigLoader::discover_packages(temp_dir.path()).unwrap();

        assert!(!result.packages.is_empty());
    }
}
