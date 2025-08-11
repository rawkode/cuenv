//! Pattern matching utilities for environment variable filtering

use crate::errors::{Error, RecoveryHint, Result};
use regex::Regex;

/// Pattern matcher for environment variable names
pub struct PatternMatcher;

impl PatternMatcher {
    /// Compile a single pattern with error handling
    pub fn compile_pattern(pattern: &str) -> Result<Regex> {
        // Convert glob-style patterns to regex
        let regex_pattern = if pattern.contains('*') || pattern.contains('?') {
            // Convert glob pattern to regex
            let escaped = regex::escape(pattern);
            // Replace escaped glob characters with regex equivalents
            let regex_pattern = escaped.replace(r"\*", ".*").replace(r"\?", ".");
            // Anchor the pattern to match the entire string
            format!("^{regex_pattern}$")
        } else {
            // Exact match for patterns without wildcards
            format!("^{}$", regex::escape(pattern))
        };

        Regex::new(&regex_pattern).map_err(|e| Error::Configuration {
            message: format!("Invalid pattern '{pattern}': {e}"),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check the glob pattern syntax".to_string(),
            },
        })
    }

    /// Check if a variable name matches a pattern (supports wildcards)
    pub fn matches_pattern(var_name: &str, pattern: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix('*') {
            var_name.starts_with(prefix)
        } else if let Some(suffix) = pattern.strip_prefix('*') {
            var_name.ends_with(suffix)
        } else if pattern.contains('*') {
            // Simple glob pattern matching
            let regex_pattern = pattern.replace('*', ".*");
            if let Ok(regex) = Regex::new(&format!("^{regex_pattern}$")) {
                regex.is_match(var_name)
            } else {
                false
            }
        } else {
            var_name == pattern
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(PatternMatcher::matches_pattern("PATH", "PATH"));
        assert!(PatternMatcher::matches_pattern("CARGO_HOME", "CARGO_*"));
        assert!(PatternMatcher::matches_pattern("NODE_ENV", "NODE_*"));
        assert!(PatternMatcher::matches_pattern("npm_config_cache", "npm_*"));
        assert!(!PatternMatcher::matches_pattern("PATH", "HOME"));
        assert!(!PatternMatcher::matches_pattern("CARGO_HOME", "NODE_*"));
    }

    #[test]
    fn test_compile_pattern() {
        // Test exact pattern
        let regex = PatternMatcher::compile_pattern("PATH").unwrap();
        assert!(regex.is_match("PATH"));
        assert!(!regex.is_match("PATHS"));

        // Test wildcard pattern
        let regex = PatternMatcher::compile_pattern("CARGO_*").unwrap();
        assert!(regex.is_match("CARGO_HOME"));
        assert!(regex.is_match("CARGO_TARGET_DIR"));
        assert!(!regex.is_match("RUSTC"));
    }
}
