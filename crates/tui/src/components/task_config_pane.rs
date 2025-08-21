use crate::events::{TaskInfo, TaskState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

pub struct TaskConfigPane {
    current_task: Option<String>,
    current_task_info: Option<TaskInfo>,
    scroll_offset: u16,
}

impl TaskConfigPane {
    pub fn new() -> Self {
        Self {
            current_task: None,
            current_task_info: None,
            scroll_offset: 0,
        }
    }

    pub fn set_task(&mut self, task_name: String, task_info: Option<TaskInfo>) {
        if self.current_task.as_ref() != Some(&task_name) {
            self.current_task = Some(task_name);
            self.current_task_info = task_info;
            self.scroll_offset = 0;
        } else if task_info.is_some() {
            self.current_task_info = task_info;
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.render_with_focus(frame, area, false);
    }

    pub fn render_with_focus(&mut self, frame: &mut Frame<'_>, area: Rect, focused: bool) {
        let border_style = if focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(" Task Configuration ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if let Some(task_info) = &self.current_task_info {
            self.render_task_config(frame, inner_area, task_info);
        } else if self.current_task.is_some() {
            let loading = Paragraph::new("Loading task configuration...")
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true });
            frame.render_widget(loading, inner_area);
        } else {
            let no_task = Paragraph::new("No task selected")
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true });
            frame.render_widget(no_task, inner_area);
        }
    }

    fn render_task_config(&self, frame: &mut Frame<'_>, area: Rect, task_info: &TaskInfo) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Basic info
                Constraint::Min(0),    // Details table
            ])
            .split(area);

        // Basic task info
        self.render_basic_info(frame, chunks[0], task_info);

        // Detailed configuration table
        self.render_details_table(frame, chunks[1], task_info);
    }

    fn render_basic_info(&self, frame: &mut Frame<'_>, area: Rect, task_info: &TaskInfo) {
        let status_style = match task_info.state {
            TaskState::Queued => Style::default().fg(Color::Yellow),
            TaskState::Running => Style::default().fg(Color::Blue),
            TaskState::Completed => Style::default().fg(Color::Green),
            TaskState::Failed => Style::default().fg(Color::Red),
            TaskState::Cancelled => Style::default().fg(Color::DarkGray),
        };

        let status_text = match task_info.state {
            TaskState::Queued => "⏳ Queued",
            TaskState::Running => "🔄 Running",
            TaskState::Completed => "✅ Completed",
            TaskState::Failed => "❌ Failed",
            TaskState::Cancelled => "⊘ Cancelled",
        };

        let lines = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Cyan)),
                Span::raw(&task_info.name),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Cyan)),
                Span::styled(status_text, status_style),
            ]),
            Line::from(vec![
                Span::styled("Dependencies: ", Style::default().fg(Color::Cyan)),
                Span::raw(if task_info.dependencies.is_empty() {
                    "None".to_string()
                } else {
                    task_info.dependencies.join(", ")
                }),
            ]),
        ];

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_details_table(&self, frame: &mut Frame<'_>, area: Rect, task_info: &TaskInfo) {
        let mut rows = Vec::new();

        // Task metadata
        rows.push(Row::new(vec![
            Cell::from("Metadata").style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from(""),
        ]));

        if let Some(duration) = task_info.duration() {
            let formatted_time = format!("{:.2}s", duration.as_secs_f64());
            rows.push(Row::new(vec![
                Cell::from("  Runtime").style(Style::default().fg(Color::Cyan)),
                Cell::from(formatted_time),
            ]));
        }

        rows.push(Row::new(vec![
            Cell::from("  Log Entries").style(Style::default().fg(Color::Cyan)),
            Cell::from(task_info.logs.len().to_string()),
        ]));

        if let Some(exit_code) = task_info.exit_code {
            rows.push(Row::new(vec![
                Cell::from("  Exit Code").style(Style::default().fg(Color::Cyan)),
                Cell::from(exit_code.to_string()),
            ]));
        }

        let table =
            Table::new(rows, [Constraint::Length(20), Constraint::Min(0)]).style(Style::default());

        frame.render_widget(table, area);
    }

    fn format_env_value(&self, value: &str) -> (String, Style) {
        if value.starts_with("${") && value.ends_with("}") {
            (
                format!("🔒 {}", value),
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
        } else if value.len() > 50 {
            (
                format!("{}...", &value[..47]),
                Style::default().fg(Color::White),
            )
        } else {
            (value.to_string(), Style::default().fg(Color::White))
        }
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn get_current_task(&self) -> Option<&String> {
        self.current_task.as_ref()
    }
}

impl Default for TaskConfigPane {
    fn default() -> Self {
        Self::new()
    }
}
