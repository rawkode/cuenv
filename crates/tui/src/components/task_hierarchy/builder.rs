use super::{TaskHierarchy, TreeLine};
use crate::events::TaskInfo;
use std::collections::HashMap;

impl TaskHierarchy {
    pub async fn build_tree_lines(&mut self) {
        let tasks = self.task_registry.get_all_tasks().await;

        // Parse task names to build hierarchy from dot notation
        let task_hierarchy = self.build_task_hierarchy(&tasks);

        // Preserve selection if empty: choose first root as default
        if self.selected_task.is_none() {
            if let Some(first) = task_hierarchy.keys().next() {
                self.selected_task = Some(first.clone());
            }
        }

        self.visible_lines.clear();
        self.max_line_width = 0;

        // Build tree from hierarchy
        let mut root_names: Vec<_> = task_hierarchy.keys().cloned().collect();
        root_names.sort();

        for root_name in &root_names {
            self.build_hierarchy_tree(root_name, &task_hierarchy, &tasks, 0, "", true);
        }

        // Expand all nodes by default to show structure
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
        for root_name in &root_names {
            self.build_hierarchy_tree(root_name, &task_hierarchy, &tasks, 0, "", true);
        }

        // Calculate max line width for horizontal scrolling
        for line in &self.visible_lines {
            let display_name = self.get_display_name(&line.task_name);
            let line_width = line.prefix.len()
                + (if line.has_children { 2 } else { 0 }) // expand indicator
                + 2 // icon
                + display_name.len();
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

    // Build a hierarchical structure from task groups (not execution dependencies)
    fn build_task_hierarchy(
        &self,
        tasks: &HashMap<String, TaskInfo>,
    ) -> HashMap<String, Vec<String>> {
        let mut hierarchy: HashMap<String, Vec<String>> = HashMap::new();
        let mut group_tasks: HashMap<String, Vec<String>> = HashMap::new();

        // Parse task names to identify groups and standalone tasks
        // Handle both colon (:) and dot (.) notation for task groups
        for task_name in tasks.keys() {
            if let Some(separator_pos) = task_name.find(':').or_else(|| task_name.find('.')) {
                // This is a grouped task like "count:task_0" or "count.task_0"
                let group_name = &task_name[..separator_pos];
                group_tasks
                    .entry(group_name.to_string())
                    .or_default()
                    .push(task_name.clone());
            } else {
                // This is a standalone task
                hierarchy.entry(task_name.clone()).or_default();
            }
        }

        // Add groups to hierarchy
        for (group_name, mut group_task_list) in group_tasks {
            group_task_list.sort();
            hierarchy.insert(group_name, group_task_list);
        }

        hierarchy
    }

    // Build tree from hierarchical structure
    fn build_hierarchy_tree(
        &mut self,
        task_name: &str,
        hierarchy: &HashMap<String, Vec<String>>,
        tasks: &HashMap<String, TaskInfo>,
        depth: usize,
        parent_prefix: &str,
        is_last_child: bool,
    ) {
        let children = self.get_direct_children(task_name, hierarchy, tasks);
        let has_children = !children.is_empty();
        let is_expanded = self.expanded_nodes.contains(task_name);

        let connector = if depth == 0 {
            ""
        } else if is_last_child {
            "└─ "
        } else {
            "├─ "
        };

        let prefix = format!("{parent_prefix}{connector}");

        // TODO: Get task state for status icon in async context
        let status_icon = "⏳"; // Default for now

        // Get dependency count
        let dependency_count = if let Some(task_info) = tasks.get(task_name) {
            task_info.dependencies.len()
        } else {
            0
        };

        self.visible_lines.push(TreeLine {
            task_name: task_name.to_string(),
            is_expanded,
            has_children,
            prefix: prefix.clone(),
            status_icon: status_icon.to_string(),
            dependency_count,
        });

        if is_expanded && has_children {
            let child_prefix = if depth == 0 {
                "".to_string()
            } else if is_last_child {
                format!("{parent_prefix}    ")
            } else {
                format!("{parent_prefix}│   ")
            };

            let num_children = children.len();
            for (idx, child) in children.iter().enumerate() {
                let is_last = idx == num_children - 1;
                self.build_hierarchy_tree(
                    child,
                    hierarchy,
                    tasks,
                    depth + 1,
                    &child_prefix,
                    is_last,
                );
            }
        }
    }

    // Get direct children of a task in the hierarchy
    fn get_direct_children(
        &self,
        task_name: &str,
        hierarchy: &HashMap<String, Vec<String>>,
        _tasks: &HashMap<String, TaskInfo>,
    ) -> Vec<String> {
        // Get children from dependency hierarchy only
        if let Some(hierarchy_children) = hierarchy.get(task_name) {
            hierarchy_children.clone()
        } else {
            Vec::new()
        }
    }

    // Get display name (last part after final colon or dot)
    pub fn get_display_name(&self, full_name: &str) -> String {
        if let Some(last_separator) = full_name.rfind(':').or_else(|| full_name.rfind('.')) {
            full_name[last_separator + 1..].to_string()
        } else {
            full_name.to_string()
        }
    }
}
