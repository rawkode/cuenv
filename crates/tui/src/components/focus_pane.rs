use crate::events::{LogEntry, LogStream, TaskInfo, TaskRegistry};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table, Wrap,
    },
    Frame,
};

pub struct FocusPane {
    task_registry: TaskRegistry,
    current_task: Option<String>,
    current_task_info: Option<TaskInfo>,
    log_scroll_offset: u16,
    auto_scroll: bool,
}

impl FocusPane {
    pub fn new(task_registry: TaskRegistry) -> Self {
        Self {
            task_registry,
            current_task: None,
            current_task_info: None,
            log_scroll_offset: 0,
            auto_scroll: true,
        }
    }

    pub fn set_task(&mut self, task_name: String) {
        if self.current_task.as_ref() != Some(&task_name) {
            self.current_task = Some(task_name);
            self.current_task_info = None; // Clear cached info
            self.log_scroll_offset = 0;
            self.auto_scroll = true;
        }
    }

    pub fn needs_task_info_update(&self) -> bool {
        self.current_task.is_some() && self.current_task_info.is_none()
    }

    pub async fn update_task_info(&mut self) {
        if let Some(task_name) = &self.current_task {
            // Clone the task name to avoid holding a reference during the async call
            let task_name_clone = task_name.clone();

            // Fetch task info with a timeout to prevent indefinite blocking
            let task_info_result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                self.task_registry.get_task(&task_name_clone),
            )
            .await;

            // Only update if we successfully got the task info
            if let Ok(task_info) = task_info_result {
                self.current_task_info = task_info;
            }
        }
    }

    pub fn get_current_task(&self) -> Option<&String> {
        self.current_task.as_ref()
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Task info
                Constraint::Min(0),    // Logs
            ])
            .split(area);

        // Render task info
        self.render_task_info(frame, chunks[0]);

        // Render logs
        self.render_logs(frame, chunks[1]);
    }

    fn render_task_info(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(" Task Details ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if let Some(task_info) = &self.current_task_info {
            let table = self.create_task_info_table(task_info);
            frame.render_widget(table, inner_area);
        } else if self.current_task.is_some() {
            // Task selected but info not loaded yet
            let loading =
                Paragraph::new("Loading task info...").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(loading, inner_area);
        } else {
            let empty_msg =
                Paragraph::new("No task selected").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(empty_msg, inner_area);
        }
    }

    fn create_task_info_table(&self, task: &TaskInfo) -> Table {
        let mut rows = vec![];

        // Task name and state
        rows.push(Row::new(vec![
            Cell::from("Task:").style(Style::default().fg(Color::DarkGray)),
            Cell::from(task.name.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
        ]));

        rows.push(Row::new(vec![
            Cell::from("State:").style(Style::default().fg(Color::DarkGray)),
            Cell::from(format!("{} {:?}", task.state.icon(), task.state))
                .style(self.get_state_style(&task.state)),
        ]));

        // Duration
        if let Some(duration) = task.duration() {
            rows.push(Row::new(vec![
                Cell::from("Duration:").style(Style::default().fg(Color::DarkGray)),
                Cell::from(format!("{:.2}s", duration.as_secs_f64()))
                    .style(Style::default().fg(Color::White)),
            ]));
        }

        // Exit code
        if let Some(exit_code) = task.exit_code {
            let exit_style = if exit_code == 0 {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            };
            rows.push(Row::new(vec![
                Cell::from("Exit Code:").style(Style::default().fg(Color::DarkGray)),
                Cell::from(exit_code.to_string()).style(exit_style),
            ]));
        }

        // Dependencies
        if !task.dependencies.is_empty() {
            rows.push(Row::new(vec![
                Cell::from("Dependencies:").style(Style::default().fg(Color::DarkGray)),
                Cell::from(task.dependencies.join(", ")).style(Style::default().fg(Color::Blue)),
            ]));
        }

        // Current message
        if let Some(message) = &task.message {
            rows.push(Row::new(vec![
                Cell::from("Message:").style(Style::default().fg(Color::DarkGray)),
                Cell::from(message.clone()).style(Style::default().fg(Color::Yellow)),
            ]));
        }

        Table::new(rows, [Constraint::Length(15), Constraint::Min(0)]).style(Style::default())
    }

    fn render_logs(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let block = Block::default()
            .title(format!(
                " Logs {} ",
                if self.auto_scroll {
                    "[AUTO]"
                } else {
                    "[MANUAL]"
                }
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner_area = block.inner(chunks[0]);
        frame.render_widget(block, chunks[0]);

        if let Some(task_info) = &self.current_task_info {
            let formatted_logs = self.format_logs(&task_info.logs);
            let total_lines = formatted_logs.1;
            let visible_height = inner_area.height as usize;

            // Auto-scroll to bottom if enabled
            let scroll_offset = if self.auto_scroll && total_lines > visible_height {
                (total_lines - visible_height) as u16
            } else {
                self.log_scroll_offset
            };

            let paragraph = Paragraph::new(formatted_logs.0)
                .scroll((scroll_offset, 0))
                .wrap(Wrap { trim: false });

            frame.render_widget(paragraph, inner_area);

            // Update scroll offset after rendering
            if self.auto_scroll && total_lines > visible_height {
                self.log_scroll_offset = (total_lines - visible_height) as u16;
            }

            // Render scrollbar if needed
            if total_lines > visible_height {
                let mut scrollbar_state = ScrollbarState::default()
                    .content_length(total_lines.saturating_sub(visible_height))
                    .position(self.log_scroll_offset as usize);

                let scrollbar = Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"));

                frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
            }
        } else {
            let empty_msg =
                Paragraph::new("No task selected").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(empty_msg, inner_area);
        }
    }

    fn format_logs(&self, logs: &[LogEntry]) -> (Vec<Line>, usize) {
        let mut lines = Vec::new();
        let mut line_count = 0;

        for log in logs {
            let timestamp = format!("{:>8.2}s", log.timestamp.elapsed().as_secs_f64());
            let stream_style = match log.stream {
                LogStream::Stdout => Style::default().fg(Color::White),
                LogStream::Stderr => Style::default().fg(Color::Red),
                LogStream::System => Style::default().fg(Color::Yellow),
            };

            // Split content into lines
            for content_line in log.content.lines() {
                let mut spans = vec![
                    Span::styled(timestamp.clone(), Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                ];

                match log.stream {
                    LogStream::Stdout => spans.push(Span::raw("│ ")),
                    LogStream::Stderr => {
                        spans.push(Span::styled("┃ ", Style::default().fg(Color::Red)))
                    }
                    LogStream::System => {
                        spans.push(Span::styled("┊ ", Style::default().fg(Color::Yellow)))
                    }
                }

                spans.push(Span::styled(content_line.to_string(), stream_style));
                lines.push(Line::from(spans));
                line_count += 1;
            }
        }

        (lines, line_count)
    }

    fn get_state_style(&self, state: &crate::events::TaskState) -> Style {
        match state {
            crate::events::TaskState::Queued => Style::default().fg(Color::DarkGray),
            crate::events::TaskState::Running => Style::default().fg(Color::Yellow),
            crate::events::TaskState::Completed => Style::default().fg(Color::Green),
            crate::events::TaskState::Failed => Style::default().fg(Color::Red),
            crate::events::TaskState::Cancelled => Style::default().fg(Color::Magenta),
        }
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(amount);
        self.auto_scroll = false;
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.log_scroll_offset = self.log_scroll_offset.saturating_add(amount);
        // Don't disable auto-scroll when scrolling down
    }

    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
    }

    pub fn jump_to_top(&mut self) {
        self.log_scroll_offset = 0;
        self.auto_scroll = false;
    }

    pub fn jump_to_bottom(&mut self) {
        self.auto_scroll = true;
    }
}
