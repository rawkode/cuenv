//! Configuration types and conversion for cache key filtering

use cuenv_config::CacheEnvConfig;
use serde::{Deserialize, Serialize};

/// Configuration for environment variable filtering in cache keys
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheKeyFilterConfig {
    /// Patterns to include (allowlist)
    pub include: Vec<String>,
    /// Patterns to exclude (denylist)
    pub exclude: Vec<String>,
    /// Whether to use smart defaults for common build tools
    #[serde(rename = "useSmartDefaults")]
    pub use_smart_defaults: bool,
}

impl Default for CacheKeyFilterConfig {
    fn default() -> Self {
        Self {
            include: vec![],
            exclude: vec![],
            use_smart_defaults: true,
        }
    }
}

/// Convert CUE CacheEnvConfig to CacheKeyFilterConfig
impl From<CacheEnvConfig> for CacheKeyFilterConfig {
    fn from(cue_config: CacheEnvConfig) -> Self {
        Self {
            include: cue_config.include.unwrap_or_default(),
            exclude: cue_config.exclude.unwrap_or_default(),
            use_smart_defaults: cue_config.use_smart_defaults.unwrap_or(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CacheKeyFilterConfig::default();
        assert!(config.include.is_empty());
        assert!(config.exclude.is_empty());
        assert!(config.use_smart_defaults);
    }

    #[test]
    fn test_from_cue_config() {
        let cue_config = CacheEnvConfig {
            include: Some(vec!["PATH".to_string()]),
            exclude: Some(vec!["PS1".to_string()]),
            use_smart_defaults: Some(false),
        };

        let config: CacheKeyFilterConfig = cue_config.into();
        assert_eq!(config.include, vec!["PATH"]);
        assert_eq!(config.exclude, vec!["PS1"]);
        assert!(!config.use_smart_defaults);
    }
}
