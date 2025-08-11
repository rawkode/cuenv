//! Environment variable filtering logic for cache key generation

use crate::keys::config::CacheKeyFilterConfig;
use crate::keys::filter::{PatternMatcher, SmartDefaults};
use regex::Regex;
use std::collections::HashMap;

/// Environment variable filtering implementation
pub struct FilterLogic;

impl FilterLogic {
    /// Determine if a variable should be included in the cache key
    pub fn should_include_var(
        var_name: &str,
        task_name: &str,
        config: &CacheKeyFilterConfig,
        task_patterns: &HashMap<String, (Vec<Regex>, Vec<Regex>)>,
        include_patterns: &[Regex],
        exclude_patterns: &[Regex],
    ) -> bool {
        // Get task-specific patterns if available
        let (include_patterns, exclude_patterns): (&[Regex], &[Regex]) =
            if let Some(patterns) = task_patterns.get(task_name) {
                // Use task-specific patterns
                (&patterns.0, &patterns.1)
            } else {
                // Use global patterns
                (include_patterns, exclude_patterns)
            };

        // Check exclude patterns first (denylist takes precedence)
        for pattern in exclude_patterns {
            if pattern.is_match(var_name) {
                return false;
            }
        }

        // Check include patterns
        let has_include_patterns = !config.include.is_empty();
        if has_include_patterns {
            for pattern in include_patterns {
                if pattern.is_match(var_name) {
                    return true;
                }
            }
            // If there are include patterns but none matched, exclude the variable
            return false;
        }

        // If no include patterns, use smart defaults if enabled
        if config.use_smart_defaults {
            // Also check global exclude patterns when using smart defaults
            for pattern in exclude_patterns {
                if pattern.is_match(var_name) {
                    return false;
                }
            }
            return Self::is_smart_default_var(var_name);
        }

        // If no patterns and no smart defaults, include all variables
        true
    }

    /// Check if variable matches smart default patterns
    pub fn is_smart_default_var(var_name: &str) -> bool {
        // Get the smart defaults
        let (allowlist, denylist) = SmartDefaults::get_defaults();

        // Check denylist first
        for pattern in denylist {
            if PatternMatcher::matches_pattern(var_name, pattern) {
                return false;
            }
        }

        // Check allowlist
        for pattern in allowlist {
            if PatternMatcher::matches_pattern(var_name, pattern) {
                return true;
            }
        }

        // Default to excluding variables not in allowlist
        false
    }

    /// Filter environment variables based on configured patterns
    pub fn filter_env_vars(
        task_name: &str,
        env_vars: &HashMap<String, String>,
        task_configs: &HashMap<String, CacheKeyFilterConfig>,
        global_config: &CacheKeyFilterConfig,
        task_patterns: &HashMap<String, (Vec<Regex>, Vec<Regex>)>,
        include_patterns: &[Regex],
        exclude_patterns: &[Regex],
    ) -> HashMap<String, String> {
        let mut filtered = HashMap::new();

        // Get task-specific config or fall back to global config
        let config = task_configs.get(task_name).unwrap_or(global_config);

        for (key, value) in env_vars {
            if Self::should_include_var(
                key,
                task_name,
                config,
                task_patterns,
                include_patterns,
                exclude_patterns,
            ) {
                filtered.insert(key.clone(), value.clone());
            }
        }

        filtered
    }
}
