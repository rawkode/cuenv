//! Pattern compilation utilities for cache key generation

use crate::errors::Result;
use crate::keys::config::CacheKeyFilterConfig;
use crate::keys::filter::{PatternMatcher, SmartDefaults};
use regex::Regex;

/// Pattern compilation utilities
pub struct PatternCompiler;

impl PatternCompiler {
    /// Compile patterns for a specific task configuration
    pub fn compile_task_patterns(
        config: &CacheKeyFilterConfig,
    ) -> Result<(Vec<Regex>, Vec<Regex>)> {
        let mut include_patterns = Vec::new();
        let mut exclude_patterns = Vec::new();

        // Add custom include patterns (these are specific to the task)
        for pattern in &config.include {
            let regex = PatternMatcher::compile_pattern(pattern)?;
            include_patterns.push(regex);
        }

        // Add custom exclude patterns (these are specific to the task)
        for pattern in &config.exclude {
            let regex = PatternMatcher::compile_pattern(pattern)?;
            exclude_patterns.push(regex);
        }

        Ok((include_patterns, exclude_patterns))
    }

    /// Compile patterns from a specific configuration
    pub fn compile_config_patterns(
        config: &CacheKeyFilterConfig,
        include_patterns: &mut Vec<Regex>,
        exclude_patterns: &mut Vec<Regex>,
    ) -> Result<()> {
        // Add smart defaults if enabled
        if config.use_smart_defaults {
            let (smart_allowlist, smart_denylist) = SmartDefaults::get_defaults();

            // Add smart allowlist patterns
            for pattern in smart_allowlist {
                let regex = PatternMatcher::compile_pattern(pattern)?;
                include_patterns.push(regex);
            }

            // Add smart denylist patterns
            for pattern in smart_denylist {
                let regex = PatternMatcher::compile_pattern(pattern)?;
                exclude_patterns.push(regex);
            }
        } else {
            // Use basic defaults when smart defaults are disabled
            let (_, denylist) = SmartDefaults::get_defaults();
            for pattern in denylist {
                let regex = PatternMatcher::compile_pattern(pattern)?;
                exclude_patterns.push(regex);
            }
        }

        // Add custom include patterns (these override smart defaults)
        for pattern in &config.include {
            let regex = PatternMatcher::compile_pattern(pattern)?;
            include_patterns.push(regex);
        }

        // Add custom exclude patterns (these override smart defaults)
        for pattern in &config.exclude {
            let regex = PatternMatcher::compile_pattern(pattern)?;
            exclude_patterns.push(regex);
        }

        Ok(())
    }
}
