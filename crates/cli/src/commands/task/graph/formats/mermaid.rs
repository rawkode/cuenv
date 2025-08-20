use crate::commands::task::graph::GraphFormatter;
use cuenv_core::Result;
use cuenv_task::UnifiedTaskDAG;
use std::collections::{HashMap, HashSet};

pub struct MermaidFormatter {}

impl MermaidFormatter {
    pub fn new() -> Self {
        Self {}
    }

    fn escape_node_id(&self, id: &str) -> String {
        // Replace special characters for Mermaid compatibility
        id.replace([':', '.', '-'], "_")
    }

    fn get_node_label(&self, id: &str) -> String {
        // For grouped tasks (containing ':'), show just the task name
        if let Some(colon_pos) = id.find(':') {
            id[colon_pos + 1..].to_string()
        } else {
            id.to_string()
        }
    }
}

impl GraphFormatter for MermaidFormatter {
    fn format_graph(&self, dag: &UnifiedTaskDAG, root_name: &str) -> Result<String> {
        let mut output = String::new();

        // Start Mermaid graph
        output.push_str("graph LR\n");

        let flattened = dag.get_flattened_tasks();

        // Group tasks by their group prefix (before ':')
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        let mut individual_tasks: Vec<String> = Vec::new();

        for task in flattened {
            if let Some(colon_pos) = task.id.find(':') {
                let group_name = task.id[..colon_pos].to_string();
                groups.entry(group_name).or_default().push(task.id.clone());
            } else {
                individual_tasks.push(task.id.clone());
            }
        }

        // Create subgraphs for groups
        for (group_name, group_tasks) in &groups {
            let group_id = self.escape_node_id(group_name);
            output.push_str(&format!("  subgraph {group_id}[\"Group: {group_name}\"]\n"));
            output.push_str("    direction LR\n");

            for task in group_tasks {
                let node_id = self.escape_node_id(task);
                let label = self.get_node_label(task);
                output.push_str(&format!("    {node_id}[\"{label}\"]\n"));
            }
            output.push_str("  end\n");
        }

        // Add individual tasks (not in groups), excluding the root task
        for task in &individual_tasks {
            if task != root_name {
                let node_id = self.escape_node_id(task);
                output.push_str(&format!("  {node_id}[\"{task}\"]\n"));
            }
        }

        // Add root task with special styling
        let root_node_id = self.escape_node_id(root_name);
        output.push_str(&format!("  {root_node_id}[\"{root_name}\"]\n"));

        output.push('\n');

        // Add edges (dependencies)
        let mut added_edges: HashSet<(String, String)> = HashSet::new();

        for task in flattened {
            for dep in &task.dependencies {
                let from_id = self.escape_node_id(dep);
                let to_id = self.escape_node_id(&task.id);

                // Avoid duplicate edges
                let edge = (from_id.clone(), to_id.clone());
                if !added_edges.contains(&edge) {
                    output.push_str(&format!("  {from_id} --> {to_id}\n"));
                    added_edges.insert(edge);
                }
            }
        }

        // Add styling classes
        output.push('\n');
        output.push_str("  classDef task fill:#e1f5fe\n");
        output.push_str("  classDef group stroke-dasharray: 5 5\n");
        output.push('\n');

        // Apply classes
        output.push_str(&format!("  class {root_node_id} task\n"));

        for group_name in groups.keys() {
            let group_id = self.escape_node_id(group_name);
            output.push_str(&format!("  class {group_id} group\n"));
        }

        Ok(output)
    }
}
