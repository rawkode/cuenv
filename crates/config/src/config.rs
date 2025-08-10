//! Centralized configuration management for cuenv
//!
//! This module provides the core `Config` struct that serves as the single source of truth
//! for all configuration data in cuenv. The configuration is immutable after construction
//! and can be safely shared across components.

use crate::parser::{CommandConfig, Hook, ParseResult, TaskConfig, VariableMetadata};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Immutable configuration struct that serves as the single source of truth
/// for all configuration data in cuenv.
///
/// This struct contains all parsed CUE data, runtime settings, and metadata
/// needed by various components. It's designed to be `Clone + Send + Sync`
/// for safe sharing across async tasks.
#[derive(Debug, Clone)]
pub struct Config {
    /// Selected environment name (e.g., "dev", "prod")
    pub environment_name: String,
    
    /// List of capabilities to filter configuration by
    pub capabilities: Vec<String>,
    
    /// Working directory where configuration was loaded from
    pub working_directory: PathBuf,
    
    /// Parsed CUE data containing variables, commands, tasks, and hooks
    pub parsed_data: Arc<ParseResult>,
    
    /// Original environment variables captured at startup
    pub original_environment: HashMap<String, String>,
    
    /// Runtime configuration settings
    pub runtime_settings: RuntimeSettings,
    
    /// Monorepo package information (if applicable)
    pub package_info: Option<PackageInfo>,
    
    /// Security context and access restrictions
    pub security_context: SecurityContext,
}

/// Runtime configuration settings that affect how cuenv operates
#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    /// Whether caching is enabled
    pub cache_enabled: bool,
    
    /// Cache directory path
    pub cache_directory: Option<PathBuf>,
    
    /// Whether to run in audit mode
    pub audit_mode: bool,
    
    /// Whether to run in dry run mode (for CLI completions)
    pub dry_run: bool,
    
    /// Verbosity level for logging
    pub verbosity: u8,
}

/// Information about monorepo packages and cross-package references
#[derive(Debug, Clone)]
pub struct PackageInfo {
    /// Current package name
    pub current_package: String,
    
    /// Map of package names to their directories
    pub packages: HashMap<String, PathBuf>,
    
    /// Cross-package task references
    pub cross_package_refs: HashMap<String, Vec<String>>,
}

/// Security context containing access restrictions and audit settings
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// Whether file access restrictions are enabled
    pub file_restrictions: bool,
    
    /// Whether network access restrictions are enabled
    pub network_restrictions: bool,
    
    /// List of allowed file paths for read access
    pub allowed_read_paths: Vec<PathBuf>,
    
    /// List of allowed file paths for write access
    pub allowed_write_paths: Vec<PathBuf>,
    
    /// List of denied file paths
    pub denied_paths: Vec<PathBuf>,
    
    /// List of allowed network hosts
    pub allowed_hosts: Vec<String>,
}

impl Config {
    /// Create a new Config instance
    pub fn new(
        environment_name: String,
        capabilities: Vec<String>,
        working_directory: PathBuf,
        parsed_data: ParseResult,
        original_environment: HashMap<String, String>,
        runtime_settings: RuntimeSettings,
    ) -> Self {
        Self {
            environment_name,
            capabilities,
            working_directory,
            parsed_data: Arc::new(parsed_data),
            original_environment,
            runtime_settings,
            package_info: None,
            security_context: SecurityContext::default(),
        }
    }
    
    /// Create a new Config with additional package and security information
    pub fn with_extensions(
        environment_name: String,
        capabilities: Vec<String>,
        working_directory: PathBuf,
        parsed_data: ParseResult,
        original_environment: HashMap<String, String>,
        runtime_settings: RuntimeSettings,
        package_info: Option<PackageInfo>,
        security_context: SecurityContext,
    ) -> Self {
        Self {
            environment_name,
            capabilities,
            working_directory,
            parsed_data: Arc::new(parsed_data),
            original_environment,
            runtime_settings,
            package_info,
            security_context,
        }
    }
    
    /// Get a variable value by name
    pub fn get_variable(&self, name: &str) -> Option<&String> {
        self.parsed_data.variables.get(name)
    }
    
    /// Get variable metadata by name
    pub fn get_variable_metadata(&self, name: &str) -> Option<&VariableMetadata> {
        self.parsed_data.metadata.get(name)
    }
    
    /// Get a task configuration by name
    pub fn get_task(&self, name: &str) -> Option<&TaskConfig> {
        self.parsed_data.tasks.get(name)
    }
    
    /// Get a command configuration by name
    pub fn get_command(&self, name: &str) -> Option<&CommandConfig> {
        self.parsed_data.commands.get(name)
    }
    
    /// Get hooks by type
    pub fn get_hooks(&self, hook_type: &str) -> Option<&Vec<Hook>> {
        self.parsed_data.hooks.get(hook_type)
    }
    
    /// List all available tasks
    pub fn list_tasks(&self) -> Vec<&String> {
        self.parsed_data.tasks.keys().collect()
    }
    
    /// List all available commands
    pub fn list_commands(&self) -> Vec<&String> {
        self.parsed_data.commands.keys().collect()
    }
    
    /// Filter variables by capabilities
    pub fn filter_variables_by_capabilities(&self) -> HashMap<String, String> {
        if self.capabilities.is_empty() {
            return self.parsed_data.variables.clone();
        }
        
        let mut filtered = HashMap::new();
        for (name, value) in &self.parsed_data.variables {
            if let Some(metadata) = self.parsed_data.metadata.get(name) {
                if metadata.capabilities.is_empty() || 
                   self.capabilities.iter().any(|cap| metadata.capabilities.contains(cap)) {
                    filtered.insert(name.clone(), value.clone());
                }
            } else {
                // If no metadata, include by default
                filtered.insert(name.clone(), value.clone());
            }
        }
        filtered
    }
    
    /// Filter tasks by capabilities
    pub fn filter_tasks_by_capabilities(&self) -> HashMap<String, &TaskConfig> {
        if self.capabilities.is_empty() {
            return self.parsed_data.tasks.iter().collect();
        }
        
        let mut filtered = HashMap::new();
        for (name, task) in &self.parsed_data.tasks {
            if task.capabilities.is_empty() || 
               self.capabilities.iter().any(|cap| task.capabilities.contains(cap)) {
                filtered.insert(name.clone(), task);
            }
        }
        filtered
    }
    
    /// Check if a variable is marked as sensitive
    pub fn is_variable_sensitive(&self, name: &str) -> bool {
        self.parsed_data.metadata
            .get(name)
            .map(|meta| meta.sensitive)
            .unwrap_or(false)
    }
    
    /// Get the resolved environment variables for the current environment and capabilities
    pub fn get_resolved_environment(&self) -> HashMap<String, String> {
        self.filter_variables_by_capabilities()
    }
    
    /// Check if the current configuration has any security restrictions
    pub fn has_security_restrictions(&self) -> bool {
        self.security_context.file_restrictions || self.security_context.network_restrictions
    }
    
    /// Check if this is a monorepo configuration
    pub fn is_monorepo(&self) -> bool {
        self.package_info.is_some()
    }
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            cache_directory: None,
            audit_mode: false,
            dry_run: false,
            verbosity: 0,
        }
    }
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self {
            file_restrictions: false,
            network_restrictions: false,
            allowed_read_paths: Vec::new(),
            allowed_write_paths: Vec::new(),
            denied_paths: Vec::new(),
            allowed_hosts: Vec::new(),
        }
    }
}

impl SecurityContext {
    /// Create a new SecurityContext with the specified restrictions
    pub fn new(file_restrictions: bool, network_restrictions: bool) -> Self {
        Self {
            file_restrictions,
            network_restrictions,
            ..Default::default()
        }
    }
    
    /// Check if any restrictions are enabled
    pub fn has_any_restrictions(&self) -> bool {
        self.file_restrictions || self.network_restrictions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{CommandConfig, Hook, HookType, TaskConfig, VariableMetadata};
    use std::collections::HashMap;

    fn create_test_config() -> Config {
        let mut variables = HashMap::new();
        variables.insert("TEST_VAR".to_string(), "test_value".to_string());
        
        let mut metadata = HashMap::new();
        metadata.insert("TEST_VAR".to_string(), VariableMetadata {
            description: Some("Test variable".to_string()),
            sensitive: false,
            capabilities: vec!["test".to_string()],
        });
        
        let parsed_data = ParseResult {
            variables,
            metadata,
            commands: HashMap::new(),
            tasks: HashMap::new(),
            hooks: HashMap::new(),
        };
        
        Config::new(
            "test".to_string(),
            vec!["test".to_string()],
            PathBuf::from("/test"),
            parsed_data,
            HashMap::new(),
            RuntimeSettings::default(),
        )
    }

    #[test]
    fn test_get_variable() {
        let config = create_test_config();
        assert_eq!(config.get_variable("TEST_VAR"), Some(&"test_value".to_string()));
        assert_eq!(config.get_variable("NONEXISTENT"), None);
    }

    #[test]
    fn test_get_variable_metadata() {
        let config = create_test_config();
        let metadata = config.get_variable_metadata("TEST_VAR").unwrap();
        assert_eq!(metadata.description, Some("Test variable".to_string()));
        assert!(!metadata.sensitive);
    }

    #[test]
    fn test_filter_by_capabilities() {
        let config = create_test_config();
        let filtered = config.filter_variables_by_capabilities();
        assert!(filtered.contains_key("TEST_VAR"));
        
        // Test with different capabilities
        let config_no_caps = Config::new(
            "test".to_string(),
            vec!["other".to_string()],
            PathBuf::from("/test"),
            config.parsed_data.as_ref().clone(),
            HashMap::new(),
            RuntimeSettings::default(),
        );
        let filtered_no_caps = config_no_caps.filter_variables_by_capabilities();
        assert!(!filtered_no_caps.contains_key("TEST_VAR"));
    }

    #[test]
    fn test_is_variable_sensitive() {
        let config = create_test_config();
        assert!(!config.is_variable_sensitive("TEST_VAR"));
        assert!(!config.is_variable_sensitive("NONEXISTENT"));
    }
}