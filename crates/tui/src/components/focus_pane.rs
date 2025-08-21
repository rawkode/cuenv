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

    fn create_task_info_table(&self, task: &TaskInfo) -> Table<'_> {
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

    fn format_logs(&self, logs: &[LogEntry]) -> (Vec<Line<'_>>, usize) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{LogEntry, LogStream, TaskInfo, TaskRegistry, TaskState};
    use std::time::{Duration, Instant};

    fn create_test_task_registry() -> TaskRegistry {
        TaskRegistry::new()
    }

    async fn setup_test_task_with_logs(
        registry: &TaskRegistry,
        task_name: &str,
        dependencies: Vec<String>,
        logs: Vec<LogEntry>,
    ) {
        registry
            .register_task(task_name.to_string(), dependencies)
            .await;

        // Add logs to the task directly through registry methods
        for log in logs {
            registry.add_log(task_name, log.stream, log.content).await;
        }
        registry
            .update_task_state(task_name, TaskState::Running)
            .await;
    }

    fn create_test_log_entry(content: &str, stream: LogStream, seconds_ago: u64) -> LogEntry {
        LogEntry {
            timestamp: Instant::now() - Duration::from_secs(seconds_ago),
            stream,
            content: content.to_string(),
        }
    }

    #[tokio::test]
    async fn test_focus_pane_initialization() {
        let registry = create_test_task_registry();
        let focus_pane = FocusPane::new(registry);

        assert!(focus_pane.current_task.is_none());
        assert!(focus_pane.current_task_info.is_none());
        assert_eq!(focus_pane.log_scroll_offset, 0);
        assert!(focus_pane.auto_scroll);
    }

    #[tokio::test]
    async fn test_set_task_behavior() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry);

        // Set initial task
        focus_pane.set_task("task1".to_string());
        assert_eq!(focus_pane.current_task, Some("task1".to_string()));
        assert!(focus_pane.current_task_info.is_none());
        assert_eq!(focus_pane.log_scroll_offset, 0);
        assert!(focus_pane.auto_scroll);

        // Set same task - should not reset state
        focus_pane.log_scroll_offset = 10;
        focus_pane.auto_scroll = false;
        focus_pane.set_task("task1".to_string());
        assert_eq!(focus_pane.log_scroll_offset, 10);
        assert!(!focus_pane.auto_scroll);

        // Set different task - should reset state
        focus_pane.set_task("task2".to_string());
        assert_eq!(focus_pane.current_task, Some("task2".to_string()));
        assert!(focus_pane.current_task_info.is_none());
        assert_eq!(focus_pane.log_scroll_offset, 0);
        assert!(focus_pane.auto_scroll);
    }

    #[tokio::test]
    async fn test_needs_task_info_update() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry);

        // No task selected
        assert!(!focus_pane.needs_task_info_update());

        // Task selected but no info
        focus_pane.set_task("task1".to_string());
        assert!(focus_pane.needs_task_info_update());

        // Task info available
        focus_pane.current_task_info = Some(TaskInfo::new("task1".to_string(), vec![]));
        assert!(!focus_pane.needs_task_info_update());
    }

    #[tokio::test]
    async fn test_update_task_info() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry.clone());

        // Register test task
        registry
            .register_task("test_task".to_string(), vec!["dep1".to_string()])
            .await;

        // Set task and update info
        focus_pane.set_task("test_task".to_string());
        focus_pane.update_task_info().await;

        assert!(focus_pane.current_task_info.is_some());
        let task_info = focus_pane.current_task_info.unwrap();
        assert_eq!(task_info.name, "test_task");
        assert_eq!(task_info.dependencies, vec!["dep1"]);
    }

    #[tokio::test]
    async fn test_update_task_info_timeout() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry);

        // Set non-existent task - should timeout gracefully
        focus_pane.set_task("non_existent_task".to_string());
        focus_pane.update_task_info().await;

        // Should not crash and info should remain None
        assert!(focus_pane.current_task_info.is_none());
    }

    #[tokio::test]
    async fn test_scroll_functionality() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry);

        // Test scroll up
        focus_pane.log_scroll_offset = 10;
        focus_pane.auto_scroll = true;
        focus_pane.scroll_up(5);
        assert_eq!(focus_pane.log_scroll_offset, 5);
        assert!(!focus_pane.auto_scroll); // Should disable auto scroll

        // Test scroll up with underflow protection
        focus_pane.scroll_up(10);
        assert_eq!(focus_pane.log_scroll_offset, 0);

        // Test scroll down
        focus_pane.scroll_down(3);
        assert_eq!(focus_pane.log_scroll_offset, 3);

        // Test scroll down with overflow protection
        focus_pane.scroll_down(u16::MAX);
        assert_eq!(focus_pane.log_scroll_offset, u16::MAX);
    }

    #[tokio::test]
    async fn test_jump_operations() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry);

        focus_pane.log_scroll_offset = 100;
        focus_pane.auto_scroll = true;

        // Test jump to top
        focus_pane.jump_to_top();
        assert_eq!(focus_pane.log_scroll_offset, 0);
        assert!(!focus_pane.auto_scroll);

        // Test jump to bottom
        focus_pane.log_scroll_offset = 50;
        focus_pane.auto_scroll = false;
        focus_pane.jump_to_bottom();
        assert!(focus_pane.auto_scroll);
    }

    #[tokio::test]
    async fn test_toggle_auto_scroll() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry);

        assert!(focus_pane.auto_scroll);
        focus_pane.toggle_auto_scroll();
        assert!(!focus_pane.auto_scroll);
        focus_pane.toggle_auto_scroll();
        assert!(focus_pane.auto_scroll);
    }

    #[tokio::test]
    async fn test_get_current_task() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry);

        assert!(focus_pane.get_current_task().is_none());

        focus_pane.set_task("test_task".to_string());
        assert_eq!(
            focus_pane.get_current_task(),
            Some(&"test_task".to_string())
        );
    }

    #[tokio::test]
    async fn test_log_formatting() {
        let registry = create_test_task_registry();
        let focus_pane = FocusPane::new(registry);

        let logs = vec![
            create_test_log_entry("stdout message", LogStream::Stdout, 5),
            create_test_log_entry("stderr message", LogStream::Stderr, 3),
            create_test_log_entry("system message", LogStream::System, 1),
            create_test_log_entry("multiline\nmessage\nhere", LogStream::Stdout, 0),
        ];

        let (formatted_lines, line_count) = focus_pane.format_logs(&logs);

        // Should have 6 lines total (3 single lines + 3 lines from multiline message)
        assert_eq!(line_count, 6);
        assert_eq!(formatted_lines.len(), 6);

        // Verify that each line has the correct structure (timestamp + separator + content)
        for line in &formatted_lines {
            assert!(!line.spans.is_empty());
            // Each line should have at least timestamp, space, separator, and content
            assert!(line.spans.len() >= 3);
        }
    }

    #[tokio::test]
    async fn test_log_formatting_empty() {
        let registry = create_test_task_registry();
        let focus_pane = FocusPane::new(registry);

        let (formatted_lines, line_count) = focus_pane.format_logs(&[]);
        assert_eq!(line_count, 0);
        assert!(formatted_lines.is_empty());
    }

    #[tokio::test]
    async fn test_get_state_style() {
        let registry = create_test_task_registry();
        let focus_pane = FocusPane::new(registry);

        // Test all state styles
        let queued_style = focus_pane.get_state_style(&TaskState::Queued);
        let running_style = focus_pane.get_state_style(&TaskState::Running);
        let completed_style = focus_pane.get_state_style(&TaskState::Completed);
        let failed_style = focus_pane.get_state_style(&TaskState::Failed);
        let cancelled_style = focus_pane.get_state_style(&TaskState::Cancelled);

        // Verify each style has appropriate color
        assert_eq!(queued_style.fg, Some(Color::DarkGray));
        assert_eq!(running_style.fg, Some(Color::Yellow));
        assert_eq!(completed_style.fg, Some(Color::Green));
        assert_eq!(failed_style.fg, Some(Color::Red));
        assert_eq!(cancelled_style.fg, Some(Color::Magenta));
    }

    #[tokio::test]
    async fn test_create_task_info_table_complete() {
        let registry = create_test_task_registry();
        let focus_pane = FocusPane::new(registry);

        let mut task_info = TaskInfo::new(
            "test_task".to_string(),
            vec!["dep1".to_string(), "dep2".to_string()],
        );
        task_info.state = TaskState::Completed;
        task_info.start_time = Some(Instant::now() - Duration::from_secs(10));
        task_info.end_time = Some(Instant::now() - Duration::from_secs(5));
        task_info.exit_code = Some(0);
        task_info.message = Some("Task completed successfully".to_string());

        let table = focus_pane.create_task_info_table(&task_info);

        // Verify table was created successfully
        // Note: We can't directly test table structure due to private fields
        let _table = table; // Just ensure it was created
    }

    #[tokio::test]
    async fn test_create_task_info_table_minimal() {
        let registry = create_test_task_registry();
        let focus_pane = FocusPane::new(registry);

        let task_info = TaskInfo::new("minimal_task".to_string(), vec![]);

        let table = focus_pane.create_task_info_table(&task_info);

        // Should have minimal rows: name and state only
        // Note: We can't directly test table structure due to private fields
        let _table = table; // Just ensure it was created
    }

    #[tokio::test]
    async fn test_integration_with_real_logs() {
        let registry = create_test_task_registry();
        let mut focus_pane = FocusPane::new(registry.clone());

        // Setup task with various log types
        let logs = vec![
            create_test_log_entry("Starting process", LogStream::System, 10),
            create_test_log_entry("Processing file 1", LogStream::Stdout, 8),
            create_test_log_entry("Warning: deprecated API", LogStream::Stderr, 6),
            create_test_log_entry("Processing file 2", LogStream::Stdout, 4),
            create_test_log_entry("Process completed", LogStream::System, 2),
        ];

        setup_test_task_with_logs(&registry, "integration_task", vec![], logs).await;

        // Set task and update info
        focus_pane.set_task("integration_task".to_string());
        focus_pane.update_task_info().await;

        // Verify task info is loaded
        assert!(focus_pane.current_task_info.is_some());
        let task_info = focus_pane.current_task_info.as_ref().unwrap();
        assert_eq!(task_info.logs.len(), 5);
        assert_eq!(task_info.state, TaskState::Running);

        // Test log formatting
        let (formatted_lines, line_count) = focus_pane.format_logs(&task_info.logs);
        assert_eq!(line_count, 5);
        assert_eq!(formatted_lines.len(), 5);
    }
}
