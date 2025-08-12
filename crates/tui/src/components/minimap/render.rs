use super::{MiniMap, TreeLine};
use crate::events::TaskState;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

impl MiniMap {
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

        // Task name (show only the last part after the final dot)
        let display_name = if let Some(last_dot) = tree_line.task_name.rfind('.') {
            tree_line.task_name[last_dot + 1..].to_string()
        } else {
            tree_line.task_name.clone()
        };

        let name_style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(Color::White)
        };
        spans.push(Span::styled(display_name, name_style));

        Line::from(spans)
    }
}
