use crate::commands::task::graph::GraphFormatter;
use cuenv_core::Result;
use cuenv_task::UnifiedTaskDAG;
use std::collections::{HashMap, HashSet};

pub struct DotFormatter {}

impl DotFormatter {
    pub fn new() -> Self {
        Self {}
    }

    fn escape_node_id(&self, id: &str) -> String {
        // Replace special characters that might cause issues in DOT
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

impl GraphFormatter for DotFormatter {
    fn format_graph(&self, dag: &UnifiedTaskDAG, root_name: &str) -> Result<String> {
        let mut output = String::new();

        // Start DOT graph
        output.push_str("digraph tasks {\n");
        output.push_str("  rankdir=LR;\n");
        output.push_str("  node [shape=box];\n\n");

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
            output.push_str(&format!(
                "  subgraph cluster_{} {{\n",
                self.escape_node_id(group_name)
            ));
            output.push_str(&format!("    label=\"{group_name} (group)\";\n"));
            output.push_str("    style=dashed;\n\n");

            for task in group_tasks {
                let node_id = self.escape_node_id(task);
                let label = self.get_node_label(task);
                output.push_str(&format!("    \"{node_id}\" [label=\"{label}\"];\n"));
            }
            output.push_str("  }\n\n");
        }

        // Add individual tasks (not in groups), excluding the root task
        for task in &individual_tasks {
            if task != root_name {
                let node_id = self.escape_node_id(task);
                output.push_str(&format!("  \"{node_id}\" [label=\"{task}\"];\n"));
            }
        }

        // Special styling for the root task
        let root_node_id = self.escape_node_id(root_name);
        output.push_str(&format!(
            "  \"{root_node_id}\" [label=\"{root_name}\", shape=hexagon, style=filled, fillcolor=\"#e1f5fe\"];\n\n"
        ));

        // Add edges (dependencies)
        let mut added_edges: HashSet<(String, String)> = HashSet::new();

        for task in flattened {
            for dep in &task.dependencies {
                let from_id = self.escape_node_id(dep);
                let to_id = self.escape_node_id(&task.id);

                // Avoid duplicate edges
                let edge = (from_id.clone(), to_id.clone());
                if !added_edges.contains(&edge) {
                    output.push_str(&format!("  \"{from_id}\" -> \"{to_id}\";\n"));
                    added_edges.insert(edge);
                }
            }
        }

        // Close DOT graph
        output.push_str("}\n");

        Ok(output)
    }
}
