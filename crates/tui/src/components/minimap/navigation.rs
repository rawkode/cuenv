use super::MiniMap;
use crate::events::TaskState;

impl MiniMap {
    pub fn select_next(&mut self) {
        if self.visible_lines.is_empty() {
            return;
        }
        let current_idx = self.get_selected_index();
        let next_idx = (current_idx + 1).min(self.visible_lines.len() - 1);

        if let Some(line) = self.visible_lines.get(next_idx) {
            self.selected_task = Some(line.task_name.clone());
            self.ensure_visible(next_idx);
        }
    }

    pub fn select_previous(&mut self) {
        if self.visible_lines.is_empty() {
            return;
        }
        let current_idx = self.get_selected_index();
        let prev_idx = current_idx.saturating_sub(1);

        if let Some(line) = self.visible_lines.get(prev_idx) {
            self.selected_task = Some(line.task_name.clone());
            self.ensure_visible(prev_idx);
        }
    }

    pub fn jump_to_top(&mut self) {
        if !self.visible_lines.is_empty() {
            self.selected_task = Some(self.visible_lines[0].task_name.clone());
            self.ensure_visible(0);
        }
    }

    pub fn jump_to_bottom(&mut self) {
        if !self.visible_lines.is_empty() {
            let idx = self.visible_lines.len() - 1;
            self.selected_task = Some(self.visible_lines[idx].task_name.clone());
            self.ensure_visible(idx);
        }
    }

    pub fn toggle_expand(&mut self) {
        if let Some(selected) = &self.selected_task {
            if self.expanded_nodes.contains(selected) {
                self.expanded_nodes.remove(selected);
            } else {
                self.expanded_nodes.insert(selected.clone());
            }
        }
    }

    pub fn expand_all(&mut self) {
        for line in &self.visible_lines {
            if line.has_children {
                self.expanded_nodes.insert(line.task_name.clone());
            }
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded_nodes.clear();
    }

    pub fn jump_to_first_error(&mut self) -> bool {
        for (idx, line) in self.visible_lines.iter().enumerate() {
            if self
                .cached_states
                .iter()
                .any(|(n, s)| n == &line.task_name && *s == TaskState::Failed)
            {
                self.selected_task = Some(line.task_name.clone());
                self.ensure_visible(idx);
                return true;
            }
        }
        false
    }
}