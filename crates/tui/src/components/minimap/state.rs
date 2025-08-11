use super::TreeLine;
use crate::events::{TaskRegistry, TaskState};
use std::collections::HashSet;

pub struct MiniMap {
    pub(crate) task_registry: TaskRegistry,
    pub(crate) selected_task: Option<String>,
    pub(crate) expanded_nodes: HashSet<String>,
    pub(crate) scroll_offset: u16,
    pub(crate) horizontal_scroll: u16,
    pub(crate) visible_lines: Vec<TreeLine>,
    pub(crate) max_line_width: u16,
    // Cached aggregate state per task for current frame
    pub(crate) cached_states: Vec<(String, TaskState)>,
}