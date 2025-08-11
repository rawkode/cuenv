//! Main cache key generator implementation

use crate::errors::Result;
use crate::keys::config::CacheKeyFilterConfig;
use crate::keys::filter::FilterStats;
use crate::keys::hash::HashComputer;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

mod compilation;
mod filtering;

use compilation::PatternCompiler;
use filtering::FilterLogic;

#[cfg(test)]
mod tests;

/// Cache key generator with selective environment variable filtering
pub struct CacheKeyGenerator {
    /// Global filtering configuration
    global_config: CacheKeyFilterConfig,
    /// Task-specific configurations
    task_configs: HashMap<String, CacheKeyFilterConfig>,
    /// Compiled regex patterns for performance
    include_patterns: Vec<Regex>,
    exclude_patterns: Vec<Regex>,
    /// Task-specific compiled patterns
    task_patterns: HashMap<String, (Vec<Regex>, Vec<Regex>)>,
}

impl CacheKeyGenerator {
    /// Create a new cache key generator with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(CacheKeyFilterConfig::default())
    }

    /// Create a new cache key generator with custom configuration
    pub fn with_config(config: CacheKeyFilterConfig) -> Result<Self> {
        let mut generator = Self {
            global_config: config,
            task_configs: HashMap::new(),
            include_patterns: vec![],
            exclude_patterns: vec![],
            task_patterns: HashMap::new(),
        };

        generator.compile_patterns()?;
        Ok(generator)
    }

    /// Add a task-specific configuration
    pub fn add_task_config(&mut self, task_name: &str, config: CacheKeyFilterConfig) -> Result<()> {
        // Compile task-specific patterns
        let (include_patterns, exclude_patterns) = PatternCompiler::compile_task_patterns(&config)?;
        self.task_patterns
            .insert(task_name.to_string(), (include_patterns, exclude_patterns));

        self.task_configs.insert(task_name.to_string(), config);
        Ok(())
    }

    /// Compile regex patterns for efficient matching
    fn compile_patterns(&mut self) -> Result<()> {
        self.include_patterns.clear();
        self.exclude_patterns.clear();

        // Compile global patterns only
        let global_config = self.global_config.clone();
        PatternCompiler::compile_config_patterns(
            &global_config,
            &mut self.include_patterns,
            &mut self.exclude_patterns,
        )?;

        Ok(())
    }

    /// Filter environment variables based on configured patterns
    pub fn filter_env_vars(
        &self,
        task_name: &str,
        env_vars: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        FilterLogic::filter_env_vars(
            task_name,
            env_vars,
            &self.task_configs,
            &self.global_config,
            &self.task_patterns,
            &self.include_patterns,
            &self.exclude_patterns,
        )
    }

    /// Generate a cache key for a task with selective environment variable filtering
    pub fn generate_cache_key(
        &self,
        task_name: &str,
        task_config_hash: &str,
        working_dir: &Path,
        input_files: &HashMap<String, String>,
        env_vars: &HashMap<String, String>,
        command: Option<&str>,
    ) -> Result<String> {
        // Normalize working directory
        let normalized_dir = HashComputer::normalize_working_dir(working_dir);

        // Filter environment variables
        let filtered_env = self.filter_env_vars(task_name, env_vars);

        // Compute hash
        let hash = HashComputer::compute_hash(
            task_name,
            task_config_hash,
            &normalized_dir,
            input_files,
            &filtered_env,
            command,
        );

        Ok(hash)
    }

    /// Get the effective configuration for a task
    pub fn get_task_config(&self, task_name: &str) -> &CacheKeyFilterConfig {
        self.task_configs
            .get(task_name)
            .unwrap_or(&self.global_config)
    }

    /// Get statistics about environment variable filtering
    pub fn get_filtering_stats(
        &self,
        task_name: &str,
        env_vars: &HashMap<String, String>,
    ) -> FilterStats {
        let filtered = self.filter_env_vars(task_name, env_vars);
        FilterStats {
            total_vars: env_vars.len(),
            filtered_vars: filtered.len(),
            excluded_vars: env_vars.len() - filtered.len(),
        }
    }
}
