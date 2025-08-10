use crate::events::{TaskInfo, TaskRegistry, TaskState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use std::collections::{HashMap, HashSet};

pub struct MiniMap {
    task_registry: TaskRegistry,
    selected_task: Option<String>,
    expanded_nodes: HashSet<String>,
    scroll_offset: u16,
    horizontal_scroll: u16,
    visible_lines: Vec<TreeLine>,
    max_line_width: u16,
    // Cached aggregate state per task for current frame
    cached_states: Vec<(String, TaskState)>,
}

#[derive(Clone)]
struct TreeLine {
    task_name: String,
    is_expanded: bool,
    has_children: bool,
    prefix: String,
}

impl MiniMap {
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

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let block = Block::default()
            .title(format!(
                " Task Tree {} ",
                if self.horizontal_scroll > 0 {
                    format!("[→{}]", self.horizontal_scroll)
                } else {
                    "".to_string()
                }
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner_area = block.inner(chunks[0]);
        frame.render_widget(block, chunks[0]);

        // Ensure the selected node is horizontally visible
        self.ensure_selected_visible_horizontally(inner_area.width);

        // Render tree content
        let visible_height = inner_area.height as usize;
        let visible_width = inner_area.width as usize;
        let total_lines = self.visible_lines.len();

        let mut lines = Vec::new();
        let start_idx = self.scroll_offset as usize;
        let end_idx = (start_idx + visible_height).min(total_lines);

        for idx in start_idx..end_idx {
            if let Some(tree_line) = self.visible_lines.get(idx) {
                let is_selected = self
                    .selected_task
                    .as_ref()
                    .map(|s| s == &tree_line.task_name)
                    .unwrap_or(false);

                // Lookup cached state
                let task_state = self
                    .cached_states
                    .iter()
                    .find(|(n, _)| n == &tree_line.task_name)
                    .map(|(_, s)| s.clone())
                    .unwrap_or(TaskState::Queued);

                let line = Self::render_tree_line_pure(tree_line, is_selected, task_state);
                let scrolled_line = self.apply_horizontal_scroll(line, visible_width);
                lines.push(scrolled_line);
            }
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner_area);

        // Render scrollbar if needed
        if total_lines > visible_height {
            let mut scrollbar_state = ScrollbarState::default()
                .content_length(total_lines.saturating_sub(visible_height))
                .position(self.scroll_offset as usize);

            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
        }
    }

    fn render_tree_line_pure(
        tree_line: &TreeLine,
        is_selected: bool,
        task_state: TaskState,
    ) -> Line<'static> {
        let mut spans = vec![];

        // Prefix with tree structure
        spans.push(Span::raw(tree_line.prefix.clone()));

        // Expand/collapse indicator
        if tree_line.has_children {
            let indicator = if tree_line.is_expanded {
                "▼ "
            } else {
                "▶ "
            };
            spans.push(Span::styled(
                indicator,
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Task state icon
        let icon = task_state.icon();
        let icon_color = match task_state {
            TaskState::Queued => Color::DarkGray,
            TaskState::Running => Color::Yellow,
            TaskState::Completed => Color::Green,
            TaskState::Failed => Color::Red,
            TaskState::Cancelled => Color::Magenta,
        };
        spans.push(Span::styled(
            format!("{icon} "),
            Style::default().fg(icon_color),
        ));

        // Task name
        let name_style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(Color::White)
        };
        spans.push(Span::styled(tree_line.task_name.clone(), name_style));

        Line::from(spans)
    }

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

    pub fn get_selected_task(&self) -> Option<&String> {
        self.selected_task.as_ref()
    }

    fn get_selected_index(&self) -> usize {
        self.selected_task
            .as_ref()
            .and_then(|selected| {
                self.visible_lines
                    .iter()
                    .position(|line| &line.task_name == selected)
            })
            .unwrap_or(0)
    }

    fn ensure_visible(&mut self, idx: usize) {
        let idx = idx as u16;
        if idx < self.scroll_offset {
            self.scroll_offset = idx;
        } else if idx >= self.scroll_offset + 20 {
            // Assuming roughly 20 visible lines
            self.scroll_offset = idx.saturating_sub(19);
        }
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        let max_scroll = (self.visible_lines.len() as u16).saturating_sub(20);
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
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

    fn apply_horizontal_scroll(&self, line: Line, visible_width: usize) -> Line<'static> {
        let mut total_width = 0;
        let mut visible_spans = Vec::new();
        let scroll_offset = self.horizontal_scroll as usize;

        for span in line.spans {
            let span_text = span.content.to_string();
            let span_width = unicode_width::UnicodeWidthStr::width(span_text.as_str());

            if total_width + span_width <= scroll_offset {
                total_width += span_width;
                continue;
            }

            if total_width >= scroll_offset + visible_width {
                break;
            }

            let start_in_span = scroll_offset.saturating_sub(total_width);
            let visible_start = start_in_span.min(span_text.len());
            let remaining_width = visible_width
                - visible_spans
                    .iter()
                    .map(|s: &Span| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
                    .sum::<usize>();

            let visible_text = span_text
                .chars()
                .skip(visible_start)
                .take(remaining_width)
                .collect::<String>();

            if !visible_text.is_empty() {
                visible_spans.push(Span::styled(visible_text, span.style));
            }

            total_width += span_width;
        }

        if self.horizontal_scroll > 0 && visible_spans.is_empty() {
            visible_spans.push(Span::styled("←", Style::default().fg(Color::DarkGray)));
        }

        Line::from(visible_spans)
    }

    pub fn scroll_left(&mut self, amount: u16) {
        self.horizontal_scroll = self.horizontal_scroll.saturating_sub(amount);
    }

    pub fn scroll_right(&mut self, amount: u16, area_width: u16) {
        if self.max_line_width > area_width {
            let max_scroll = self.max_line_width.saturating_sub(area_width);
            self.horizontal_scroll = (self.horizontal_scroll + amount).min(max_scroll);
        }
    }

    pub fn ensure_selected_visible_horizontally(&mut self, area_width: u16) {
        if let Some(selected) = &self.selected_task {
            if let Some(line) = self.visible_lines.iter().find(|l| l.task_name == *selected) {
                let line_start = line.prefix.len() as u16;
                let line_end = line_start + line.task_name.len() as u16 + 4; // icon and spacing

                if line_start < self.horizontal_scroll {
                    self.horizontal_scroll = line_start;
                } else if line_end > self.horizontal_scroll + area_width {
                    self.horizontal_scroll = line_end.saturating_sub(area_width);
                }
            }
        }
    }
}
