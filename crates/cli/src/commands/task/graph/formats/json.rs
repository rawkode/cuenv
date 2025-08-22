use crate::commands::task::graph::GraphFormatter;
use cuenv_core::Result;
use cuenv_task::TaskDAG;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct JsonFormatter {}

impl JsonFormatter {
    pub fn new() -> Self {
        Self {}
    }
}

impl GraphFormatter for JsonFormatter {
    fn format_graph(&self, dag: &TaskDAG, root_name: &str) -> Result<String> {
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
            }
            Err(e) => {
                tracing::warn!("Failed to get execution levels: {}", e);
                json_output["execution_levels"] = Value::Array(vec![]);
            }
        }

        // Get edges for dependencies visualization (from task to its dependencies)
        let mut edges = Vec::new();
        let flattened_tasks = dag.get_flattened_tasks();

        // Also extract all dependencies
        let mut all_dependencies = std::collections::HashSet::new();

        for task in &flattened_tasks {
            for dep in &task.dependencies {
                all_dependencies.insert(dep.clone());
                // Edge goes from task to dependency
                edges.push(json!({
                    "from": task.id,
                    "to": dep,
                    "type": "dependency"
                }));
            }
        }

        json_output["dependencies"] = json!(all_dependencies.into_iter().collect::<Vec<_>>());
        json_output["edges"] = json!(edges);

        // Create groups structure for UI organization
        let mut groups: HashMap<String, Value> = HashMap::new();

        // Process all tasks to identify groups (tasks with colons)
        for task in &flattened_tasks {
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
                    if let Some(Value::Array(ref mut tasks)) = group_tasks.get_mut("tasks") {
                        tasks.push(Value::String(task_name.to_string()));
                    }
                }
            }
        }

        json_output["groups"] = json!(groups);

        Ok(serde_json::to_string_pretty(&json_output)?)
    }
}
