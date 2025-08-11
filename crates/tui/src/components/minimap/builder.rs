use super::{MiniMap, TreeLine};
use crate::events::TaskInfo;
use std::collections::{HashMap, HashSet};

impl MiniMap {
    pub async fn build_tree_lines(&mut self) {
        let tasks = self.task_registry.get_all_tasks().await;
        let root_tasks = self.find_root_tasks(&tasks);

        // Preserve selection if empty: choose first root as default
        if self.selected_task.is_none() {
            if let Some(first) = root_tasks.first() {
                self.selected_task = Some(first.clone());
            }
        }

        self.visible_lines.clear();
        self.max_line_width = 0;

        for root_task in &root_tasks {
            self.build_tree_recursive(root_task, &tasks, 0, "", true);
        }

        // Expand all nodes by default to show dependencies
        let nodes_to_expand: Vec<String> = self
            .visible_lines
            .iter()
            .filter(|line| line.has_children)
            .map(|line| line.task_name.clone())
            .collect();

        for node in nodes_to_expand {
            self.expanded_nodes.insert(node);
        }

        // Rebuild the tree with expanded nodes
        self.visible_lines.clear();
        for root_task in &root_tasks {
            self.build_tree_recursive(root_task, &tasks, 0, "", true);
        }

        // Calculate max line width for horizontal scrolling
        for line in &self.visible_lines {
            let line_width = line.prefix.len()
                + (if line.has_children { 2 } else { 0 }) // expand indicator
                + 2 // icon
                + line.task_name.len();
            self.max_line_width = self.max_line_width.max(line_width as u16);
        }

        // Cache aggregate states in batch to avoid per-line awaits during render
        self.cached_states.clear();
        for line in &self.visible_lines {
            let name = line.task_name.clone();
            let state = self.task_registry.get_aggregate_state(&name).await;
            self.cached_states.push((name, state));
        }
    }

    fn find_root_tasks(&self, tasks: &HashMap<String, TaskInfo>) -> Vec<String> {
        let mut roots = Vec::new();
        let all_deps: HashSet<String> = tasks
            .values()
            .flat_map(|task| task.dependencies.iter().cloned())
            .collect();

        for task_name in tasks.keys() {
            if !all_deps.contains(task_name) {
                roots.push(task_name.clone());
            }
        }

        roots.sort();
        roots
    }

    fn build_tree_recursive(
        &mut self,
        task_name: &str,
        tasks: &HashMap<String, TaskInfo>,
        depth: usize,
        parent_prefix: &str,
        is_last_child: bool,
    ) {
        if let Some(task) = tasks.get(task_name) {
            let has_children = !task.dependencies.is_empty();
            let is_expanded = self.expanded_nodes.contains(task_name);

            let connector = if depth == 0 {
                ""
            } else if is_last_child {
                "└─ "
            } else {
                "├─ "
            };

            let prefix = format!("{parent_prefix}{connector}");

            self.visible_lines.push(TreeLine {
                task_name: task_name.to_string(),
                is_expanded,
                has_children,
                prefix: prefix.clone(),
            });

            if is_expanded && has_children {
                let child_prefix = if depth == 0 {
                    "".to_string()
                } else if is_last_child {
                    format!("{parent_prefix}    ")
                } else {
                    format!("{parent_prefix}│   ")
                };

                let num_children = task.dependencies.len();
                for (idx, child) in task.dependencies.iter().enumerate() {
                    let is_last = idx == num_children - 1;
                    self.build_tree_recursive(child, tasks, depth + 1, &child_prefix, is_last);
                }
            }
        }
    }
}