mod builder;
mod navigation;
mod render;
mod scroll;
mod state;

pub use state::TaskHierarchy;

use crate::events::TaskRegistry;
use std::collections::HashSet;

#[derive(Clone)]
pub(crate) struct TreeLine {
    pub task_name: String,
    pub is_expanded: bool,
    pub has_children: bool,
    pub prefix: String,
    pub status_icon: String,
    pub dependency_count: usize,
}

impl TaskHierarchy {
    pub fn new(task_registry: TaskRegistry) -> Self {
        Self {
            task_registry,
            selected_task: None,
            expanded_nodes: HashSet::new(),
            scroll_offset: 0,
            horizontal_scroll: 0,
            visible_lines: Vec::new(),
            max_line_width: 0,
            cached_states: Vec::new(),
        }
    }

    pub fn get_selected_task(&self) -> Option<&String> {
        self.selected_task.as_ref()
    }

    pub(crate) fn get_selected_index(&self) -> usize {
        self.selected_task
            .as_ref()
            .and_then(|selected| {
                self.visible_lines
                    .iter()
                    .position(|line| &line.task_name == selected)
            })
            .unwrap_or(0)
    }

    pub(crate) fn ensure_visible(&mut self, idx: usize) {
        let idx = idx as u16;
        if idx < self.scroll_offset {
            self.scroll_offset = idx;
        } else if idx >= self.scroll_offset + 20 {
            // Assuming roughly 20 visible lines
            self.scroll_offset = idx.saturating_sub(19);
        }
    }
}
