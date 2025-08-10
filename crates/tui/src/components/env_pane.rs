use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
    Frame,
};
use std::collections::HashMap;

pub struct EnvPane {
    env_vars: HashMap<String, String>,
    scroll_offset: u16,
    selected_index: Option<usize>,
    sorted_keys: Vec<String>,
}

impl EnvPane {
    pub fn new(env_vars: HashMap<String, String>) -> Self {
        let mut sorted_keys: Vec<String> = env_vars.keys().cloned().collect();
        sorted_keys.sort();

        Self {
            env_vars,
            scroll_offset: 0,
            selected_index: if sorted_keys.is_empty() {
                None
            } else {
                Some(0)
            },
            sorted_keys,
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let block = Block::default()
            .title(" Environment Variables ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner_area = block.inner(chunks[0]);
        frame.render_widget(block, chunks[0]);

        if self.sorted_keys.is_empty() {
            let empty_msg = Paragraph::new("No environment variables defined")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(empty_msg, inner_area);
            return;
        }

        // Create table rows
        let visible_height = inner_area.height as usize;
        let start_idx = self.scroll_offset as usize;
        let end_idx = (start_idx + visible_height).min(self.sorted_keys.len());

        let mut rows = Vec::new();
        for (idx, key) in self.sorted_keys[start_idx..end_idx].iter().enumerate() {
            let global_idx = start_idx + idx;
            let value = self.env_vars.get(key).map(String::as_str).unwrap_or("");

            // Check if value looks like a secret reference
            let (value_display, value_style) = if value.starts_with("${") && value.ends_with("}") {
                (
                    format!("🔒 {value}"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::ITALIC),
                )
            } else if value.contains("***") || value.contains("REDACTED") {
                (
                    "🔒 [REDACTED]".to_string(),
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::ITALIC),
                )
            } else {
                (value.to_string(), Style::default().fg(Color::White))
            };

            let is_selected = self.selected_index == Some(global_idx);
            let key_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(Color::Cyan)
            };

            rows.push(Row::new(vec![
                Cell::from(key.clone()).style(key_style),
                Cell::from(value_display).style(value_style),
            ]));
        }

        let table =
            Table::new(rows, [Constraint::Length(30), Constraint::Min(0)]).style(Style::default());

        frame.render_widget(table, inner_area);

        // Render scrollbar if needed
        if self.sorted_keys.len() > visible_height {
            let mut scrollbar_state = ScrollbarState::default()
                .content_length(self.sorted_keys.len().saturating_sub(visible_height))
                .position(self.scroll_offset as usize);

            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
        }
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
        if let Some(selected) = self.selected_index {
            self.selected_index = Some(selected.saturating_sub(amount as usize));
        }
    }

    pub fn scroll_down(&mut self, amount: u16) {
        let max_scroll = self.sorted_keys.len().saturating_sub(20) as u16;
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);

        if let Some(selected) = self.selected_index {
            let new_selected = (selected + amount as usize).min(self.sorted_keys.len() - 1);
            self.selected_index = Some(new_selected);
        }
    }

    pub fn select_next(&mut self) {
        if let Some(selected) = self.selected_index {
            if selected < self.sorted_keys.len() - 1 {
                self.selected_index = Some(selected + 1);
                self.ensure_selected_visible();
            }
        }
    }

    pub fn select_previous(&mut self) {
        if let Some(selected) = self.selected_index {
            if selected > 0 {
                self.selected_index = Some(selected - 1);
                self.ensure_selected_visible();
            }
        }
    }

    fn ensure_selected_visible(&mut self) {
        if let Some(selected) = self.selected_index {
            let selected = selected as u16;
            let visible_height = 20; // Approximate visible height

            if selected < self.scroll_offset {
                self.scroll_offset = selected;
            } else if selected >= self.scroll_offset + visible_height {
                self.scroll_offset = selected.saturating_sub(visible_height - 1);
            }
        }
    }

    pub fn get_selected(&self) -> Option<(&String, &String)> {
        self.selected_index.and_then(|idx| {
            self.sorted_keys
                .get(idx)
                .and_then(|key| self.env_vars.get(key).map(|value| (key, value)))
        })
    }
}
