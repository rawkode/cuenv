//! Task configuration types

use super::{CacheEnvConfig, SecurityConfig, TaskCacheConfig};
use serde::{de::MapAccess, de::Visitor, Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskConfig {
    pub description: Option<String>,
    pub command: Option<String>,
    pub script: Option<String>,
    pub dependencies: Option<Vec<String>>,
    #[serde(rename = "workingDir")]
    pub working_dir: Option<String>,
    pub shell: Option<String>,
    pub inputs: Option<Vec<String>>,
    pub outputs: Option<Vec<String>>,
    pub security: Option<SecurityConfig>,
    /// Cache configuration for this task (simple boolean or advanced config)
    #[serde(default, deserialize_with = "deserialize_cache_config")]
    pub cache: Option<TaskCacheConfig>,
    /// Custom cache key - if not provided, will be derived from inputs
    #[serde(rename = "cacheKey")]
    pub cache_key: Option<String>,
    /// Cache environment variable filtering configuration (deprecated, use cache.env instead)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_env: Option<CacheEnvConfig>,
    /// Timeout for task execution in seconds
    pub timeout: Option<u32>,
}

/// Custom deserializer for cache configuration to support both simple and advanced forms
fn deserialize_cache_config<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<TaskCacheConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    struct CacheConfigVisitor;

    impl<'de> Visitor<'de> for CacheConfigVisitor {
        type Value = Option<TaskCacheConfig>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a boolean, an object with cache configuration, or null")
        }

        fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E> {
            Ok(Some(TaskCacheConfig::Simple(value)))
        }

        fn visit_map<M>(self, mut map: M) -> std::result::Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut enabled = None;
            let mut env = None;

            while let Some(key) = map.next_key::<String>()? {
                match key.as_str() {
                    "enabled" => {
                        enabled = Some(map.next_value()?);
                    }
                    "env" => {
                        env = Some(map.next_value()?);
                    }
                    _ => {
                        // Skip unknown fields for forward compatibility
                        map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
            }

            // If we have an object, treat it as advanced configuration
            let enabled = enabled.unwrap_or(true); // Default to enabled
            Ok(Some(TaskCacheConfig::Advanced { enabled, env }))
        }
    }

    // Handle any value type including null/missing
    deserializer.deserialize_any(CacheConfigVisitor)
}
