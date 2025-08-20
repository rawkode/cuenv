use crate::commands::task::graph::GraphFormatter;
use cuenv_core::Result;
use cuenv_task::UnifiedTaskDAG;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct JsonFormatter {}

impl JsonFormatter {
    pub fn new() -> Self {
        Self {}
    }
}

impl GraphFormatter for JsonFormatter {
    fn format_graph(&self, dag: &UnifiedTaskDAG, root_name: &str) -> Result<String> {
        let mut json_output = json!({
            "task": root_name,
            "type": "task"
        });

        // Get execution levels
        match dag.get_execution_levels() {
            Ok(levels) => {
                let mut execution_levels = Vec::new();
                for (level_num, level_tasks) in levels.iter().enumerate() {
                    execution_levels.push(json!({
                        "level": level_num + 1,
                        "tasks": level_tasks
                    }));
                }
                json_output["execution_levels"] = Value::Array(execution_levels);

                // Create edges array
                let mut edges = Vec::new();
                let flattened = dag.get_flattened_tasks();

                for task in flattened {
                    for dep in &task.dependencies {
                        edges.push(json!({
                            "from": dep,
                            "to": task.id,
                            "type": "dependency"
                        }));
                    }
                }
                json_output["edges"] = Value::Array(edges);

                // Add dependencies for root task
                if let Some(root_deps) = dag.get_task_dependencies(root_name) {
                    json_output["dependencies"] =
                        Value::Array(root_deps.iter().map(|d| Value::String(d.clone())).collect());
                }

                // Group information (if applicable)
                let mut groups = HashMap::new();

                // For now, we'll identify groups based on task naming patterns
                for task in flattened {
                    if let Some(colon_pos) = task.id.find(':') {
                        let group_name = &task.id[..colon_pos];
                        let task_name = &task.id[colon_pos + 1..];

                        groups.entry(group_name.to_string()).or_insert_with(|| {
                            json!({
                                "mode": "unknown",
                                "tasks": Vec::<String>::new()
                            })
                        });

                        if let Some(group_tasks) = groups.get_mut(group_name) {
                            if let Some(Value::Array(ref mut tasks)) = group_tasks.get_mut("tasks")
                            {
                                tasks.push(Value::String(task_name.to_string()));
                            }
                        }
                    }
                }

                if !groups.is_empty() {
                    json_output["groups"] = Value::Object(groups.into_iter().collect());
                }
            }
            Err(e) => {
                json_output["error"] =
                    Value::String(format!("Error building execution graph: {e}"));
            }
        }

        serde_json::to_string_pretty(&json_output)
            .map_err(|e| cuenv_core::Error::configuration(format!("JSON serialization error: {e}")))
    }
}
