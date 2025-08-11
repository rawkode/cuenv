//! Task configuration types

use super::{CacheEnvConfig, SecurityConfig, TaskCacheConfig};
use serde::{de::MapAccess, de::Visitor, Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;

/// A task node that can be either a single task or a group of tasks
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TaskNode {
    /// A single task definition
    Task(Box<TaskConfig>),
    /// A group of tasks with optional description
    Group {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(flatten)]
        tasks: HashMap<String, TaskNode>,
    },
}

// Custom deserializer for TaskNode to properly distinguish between Task and Group
impl<'de> Deserialize<'de> for TaskNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;

        // Check if this looks like a task (has command or script field)
        if let serde_json::Value::Object(ref map) = value {
            let has_command = map.contains_key("command");
            let has_script = map.contains_key("script");

            if has_command || has_script {
                // It's definitely a Task
                serde_json::from_value::<TaskConfig>(value)
                    .map(|config| TaskNode::Task(Box::new(config)))
                    .map_err(serde::de::Error::custom)
            } else {
                // Check if it has any non-task fields (besides description)
                let task_fields = vec![
                    "description",
                    "command",
                    "script",
                    "dependencies",
                    "workingDir",
                    "shell",
                    "inputs",
                    "outputs",
                    "security",
                    "cache",
                    "cacheKey",
                    "cache_env",
                    "timeout",
                    "args",
                ];

                let has_non_task_fields = map.keys().any(|k| !task_fields.contains(&k.as_str()));

                if has_non_task_fields {
                    // It's a Group - extract description and other fields as tasks
                    let description = map
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let mut tasks = HashMap::new();
                    for (key, val) in map {
                        if key != "description" {
                            // Recursively deserialize as TaskNode
                            if let Ok(node) = serde_json::from_value::<TaskNode>(val.clone()) {
                                tasks.insert(key.clone(), node);
                            }
                        }
                    }

                    Ok(TaskNode::Group { description, tasks })
                } else {
                    // It's a Task with only optional fields
                    serde_json::from_value::<TaskConfig>(value)
                        .map(|config| TaskNode::Task(Box::new(config)))
                        .map_err(serde::de::Error::custom)
                }
            }
        } else {
            Err(serde::de::Error::custom("Expected an object for TaskNode"))
        }
    }
}

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
