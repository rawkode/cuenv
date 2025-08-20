//! Task configuration types

use super::{CacheEnvConfig, SecurityConfig, TaskCacheConfig};
use indexmap::IndexMap;
use serde::{de::MapAccess, de::Visitor, Deserialize, Deserializer, Serialize};
use std::fmt;

/// Collection type for tasks in a group
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskCollection {
    /// Sequential execution: tasks in array form (order preserved)
    Sequential(Vec<TaskNode>),
    /// Parallel execution: tasks in object form with names (dependency-based)
    Parallel(IndexMap<String, TaskNode>),
}

impl TaskCollection {
    /// Check if the collection is empty
    pub fn is_empty(&self) -> bool {
        match self {
            TaskCollection::Sequential(tasks) => tasks.is_empty(),
            TaskCollection::Parallel(tasks) => tasks.is_empty(),
        }
    }

    /// Get the number of tasks in the collection
    pub fn len(&self) -> usize {
        match self {
            TaskCollection::Sequential(tasks) => tasks.len(),
            TaskCollection::Parallel(tasks) => tasks.len(),
        }
    }

    /// Iterate over task names and nodes
    pub fn iter(&self) -> Box<dyn Iterator<Item = (String, &TaskNode)> + '_> {
        match self {
            TaskCollection::Sequential(tasks) => Box::new(
                tasks
                    .iter()
                    .enumerate()
                    .map(|(i, task)| (format!("task_{i}"), task)),
            ),
            TaskCollection::Parallel(tasks) => {
                Box::new(tasks.iter().map(|(name, task)| (name.clone(), task)))
            }
        }
    }

    /// Check if this is a sequential collection
    pub fn is_sequential(&self) -> bool {
        matches!(self, TaskCollection::Sequential(_))
    }

    /// Check if this is a parallel collection
    pub fn is_parallel(&self) -> bool {
        matches!(self, TaskCollection::Parallel(_))
    }
}

/// A task node that can be either a single task or a group of tasks
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TaskNode {
    /// A single task definition
    Task(Box<TaskConfig>),
    /// A group of tasks with optional description
    Group {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        tasks: TaskCollection,
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
                // Check if it has a tasks field
                if let Some(tasks_value) = map.get("tasks") {
                    // It's a Group - extract description and tasks
                    let description = map
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let tasks = match tasks_value {
                        serde_json::Value::Array(arr) => {
                            // Sequential: array of tasks
                            let mut task_nodes = Vec::new();
                            for task_val in arr {
                                if let Ok(node) =
                                    serde_json::from_value::<TaskNode>(task_val.clone())
                                {
                                    task_nodes.push(node);
                                }
                            }
                            TaskCollection::Sequential(task_nodes)
                        }
                        serde_json::Value::Object(obj) => {
                            // Parallel: object of named tasks
                            let mut task_map = IndexMap::new();
                            for (key, val) in obj {
                                if let Ok(node) = serde_json::from_value::<TaskNode>(val.clone()) {
                                    task_map.insert(key.clone(), node);
                                }
                            }
                            TaskCollection::Parallel(task_map)
                        }
                        _ => {
                            return Err(serde::de::Error::custom(
                                "tasks field must be array or object",
                            ))
                        }
                    };

                    Ok(TaskNode::Group { description, tasks })
                } else {
                    // Check if it has non-task fields (old format compatibility)
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

                    let has_non_task_fields =
                        map.keys().any(|k| !task_fields.contains(&k.as_str()));

                    if has_non_task_fields {
                        // Old format: direct task fields in object - convert to Parallel collection
                        let description = map
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        let mut tasks = IndexMap::new();
                        for (key, val) in map {
                            if key != "description" {
                                // Recursively deserialize as TaskNode
                                if let Ok(node) = serde_json::from_value::<TaskNode>(val.clone()) {
                                    tasks.insert(key.clone(), node);
                                }
                            }
                        }

                        Ok(TaskNode::Group {
                            description,
                            tasks: TaskCollection::Parallel(tasks),
                        })
                    } else {
                        // It's a Task with only optional fields
                        serde_json::from_value::<TaskConfig>(value)
                            .map(|config| TaskNode::Task(Box::new(config)))
                            .map_err(serde::de::Error::custom)
                    }
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
