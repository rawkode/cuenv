//! TUI formatter layer for rich interactive task display

use crate::{
    components::{TaskConfigPane, TaskHierarchy, TaskLogsPane, TracingPane},
    events::TaskRegistry,
    TaskEvent,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame, Terminal,
};
use std::io::{self, Stdout};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, Layer};

#[derive(Debug, Clone, PartialEq)]
pub enum FocusedPane {
    TaskHierarchy,
    TaskConfig,
    TaskLogs,
    Tracing,
}

impl FocusedPane {
    fn next(&self) -> Self {
        match self {
            Self::TaskHierarchy => Self::TaskConfig,
            Self::TaskConfig => Self::TaskLogs,
            Self::TaskLogs => Self::Tracing,
            Self::Tracing => Self::TaskHierarchy,
        }
    }

    fn previous(&self) -> Self {
        match self {
            Self::TaskHierarchy => Self::Tracing,
            Self::TaskConfig => Self::TaskHierarchy,
            Self::TaskLogs => Self::TaskConfig,
            Self::Tracing => Self::TaskLogs,
        }
    }
}

struct TuiState {
    task_hierarchy: TaskHierarchy,
    task_config_pane: TaskConfigPane,
    task_logs_pane: TaskLogsPane,
    tracing_pane: TracingPane,
    task_registry: Arc<TaskRegistry>,
    terminal: Option<Terminal<CrosstermBackend<Stdout>>>,
    focused_pane: FocusedPane,
    running: Arc<AtomicBool>,
    show_help: bool,
}

/// TUI formatter layer for interactive task display
pub struct TuiLayer {
    state: Arc<Mutex<TuiState>>,
}

impl TuiLayer {
    pub fn new() -> io::Result<Self> {
        // Setup terminal
        if let Err(e) = enable_raw_mode() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to enable raw mode: {}", e),
            ));
        }

        let mut stdout = io::stdout();
        if let Err(e) = stdout.execute(EnterAlternateScreen) {
            let _ = disable_raw_mode(); // Clean up
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to enter alternate screen: {}", e),
            ));
        }

        if let Err(e) = stdout.execute(EnableMouseCapture) {
            let _ = stdout.execute(LeaveAlternateScreen);
            let _ = disable_raw_mode();
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to enable mouse capture: {}", e),
            ));
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(term) => term,
            Err(e) => {
                let _ = io::stdout().execute(LeaveAlternateScreen);
                let _ = io::stdout().execute(DisableMouseCapture);
                let _ = disable_raw_mode();
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to create terminal: {}", e),
                ));
            }
        };

        // Create task registry
        let task_registry = Arc::new(TaskRegistry::new());

        let running = Arc::new(AtomicBool::new(true));

        let state = TuiState {
            task_hierarchy: TaskHierarchy::new((*task_registry).clone()),
            task_config_pane: TaskConfigPane::new(),
            task_logs_pane: TaskLogsPane::new(),
            tracing_pane: TracingPane::new(),
            task_registry: task_registry.clone(),
            terminal: Some(terminal),
            focused_pane: FocusedPane::TaskHierarchy,
            running: running.clone(),
            show_help: false,
        };

        let layer = Self {
            state: Arc::new(Mutex::new(state)),
        };

        // Start input handling thread
        layer.start_input_thread(running);

        // Start rendering thread
        layer.start_render_thread();

        Ok(layer)
    }

    fn start_input_thread(&self, running: Arc<AtomicBool>) {
        let state = self.state.clone();

        thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                // Poll for events with timeout
                match event::poll(Duration::from_millis(100)) {
                    Ok(true) => {
                        match event::read() {
                            Ok(event) => {
                                if let CrosstermEvent::Key(key) = event {
                                    if let Ok(mut state) = state.lock() {
                                        match key.code {
                                            KeyCode::Char('q') => {
                                                state.running.store(false, Ordering::Relaxed);
                                                break;
                                            }
                                            KeyCode::Tab => {
                                                state.focused_pane = state.focused_pane.next();
                                            }
                                            KeyCode::BackTab => {
                                                state.focused_pane = state.focused_pane.previous();
                                            }
                                            KeyCode::Char('h') => {
                                                state.show_help = !state.show_help;
                                            }
                                            KeyCode::Char('t') => {
                                                state.focused_pane = FocusedPane::Tracing;
                                            }
                                            KeyCode::Char('l') => {
                                                state.focused_pane = FocusedPane::TaskLogs;
                                            }
                                            KeyCode::Up => {
                                                match state.focused_pane {
                                                    FocusedPane::TaskHierarchy => {
                                                        state.task_hierarchy.move_up();
                                                        // Update other panes when selection changes
                                                        if let Some(selected) =
                                                            state.task_hierarchy.get_selected_task()
                                                        {
                                                            let selected = selected.clone();
                                                            let registry =
                                                                state.task_registry.clone();
                                                            tokio::spawn(async move {
                                                                if let Some(_task_info) = registry
                                                                    .get_task(&selected)
                                                                    .await
                                                                {
                                                                    // Update will happen on next render cycle
                                                                }
                                                            });
                                                        }
                                                    }
                                                    FocusedPane::TaskLogs => {
                                                        state.task_logs_pane.scroll_up(1);
                                                    }
                                                    FocusedPane::TaskConfig => {
                                                        state.task_config_pane.scroll_up(1);
                                                    }
                                                    FocusedPane::Tracing => {
                                                        state.tracing_pane.scroll_up(1);
                                                    }
                                                }
                                            }
                                            KeyCode::Down => {
                                                match state.focused_pane {
                                                    FocusedPane::TaskHierarchy => {
                                                        state.task_hierarchy.move_down();
                                                        // Update other panes when selection changes
                                                        if let Some(selected) =
                                                            state.task_hierarchy.get_selected_task()
                                                        {
                                                            let selected = selected.clone();
                                                            let registry =
                                                                state.task_registry.clone();
                                                            tokio::spawn(async move {
                                                                if let Some(_task_info) = registry
                                                                    .get_task(&selected)
                                                                    .await
                                                                {
                                                                    // Update will happen on next render cycle
                                                                }
                                                            });
                                                        }
                                                    }
                                                    FocusedPane::TaskLogs => {
                                                        state.task_logs_pane.scroll_down(1);
                                                    }
                                                    FocusedPane::TaskConfig => {
                                                        state.task_config_pane.scroll_down(1);
                                                    }
                                                    FocusedPane::Tracing => {
                                                        state.tracing_pane.scroll_down(1);
                                                    }
                                                }
                                            }
                                            KeyCode::Enter => {
                                                if matches!(
                                                    state.focused_pane,
                                                    FocusedPane::TaskHierarchy
                                                ) {
                                                    state.task_hierarchy.toggle_selected();
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Error reading input event: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(false) => {
                        // No event available, continue
                    }
                    Err(e) => {
                        eprintln!("Error polling for events: {}", e);
                        break;
                    }
                }
            }
        });
    }

    fn start_render_thread(&self) {
        let state = self.state.clone();

        thread::spawn(move || {
            let mut last_render = Instant::now();

            loop {
                if let Ok(state) = state.lock() {
                    if !state.running.load(Ordering::Relaxed) {
                        break;
                    }

                    // Render at most 30 FPS
                    if last_render.elapsed() >= Duration::from_millis(33) {
                        last_render = Instant::now();
                    }
                    drop(state); // Release lock for rendering
                }

                if let Ok(mut state) = state.lock() {
                    if let Err(e) = Self::render_static(&mut state) {
                        tracing::error!("Render error: {}", e);
                    }
                }

                thread::sleep(Duration::from_millis(16));
            }
        });
    }

    fn render_static(state: &mut TuiState) -> io::Result<()> {
        if let Some(ref mut terminal) = state.terminal {
            let focused_pane = state.focused_pane.clone();
            let show_help = state.show_help;

            terminal.draw(|f| {
                // Create a temporary reference to state for the render
                // Since we can't use std::mem::take without Default, we'll work with references
                let size = f.area();

                // Create main layout - simplified version for now
                let main_vertical = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(0),    // Top content (hierarchy + config/logs)
                        Constraint::Length(8), // Tracing pane
                        Constraint::Length(1), // Status bar
                    ])
                    .split(size);

                let top_horizontal = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(40), // Task Hierarchy
                        Constraint::Percentage(60), // Config + Logs
                    ])
                    .split(main_vertical[0]);

                let right_vertical = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(60), // Task Config
                        Constraint::Percentage(40), // Task Logs
                    ])
                    .split(top_horizontal[1]);

                // For now, render simple placeholders until we can fix the borrow issue
                use ratatui::style::{Color, Style};
                use ratatui::widgets::Paragraph;

                let hierarchy_focused = matches!(focused_pane, FocusedPane::TaskHierarchy);
                let config_focused = matches!(focused_pane, FocusedPane::TaskConfig);
                let logs_focused = matches!(focused_pane, FocusedPane::TaskLogs);
                let tracing_focused = matches!(focused_pane, FocusedPane::Tracing);

                let hierarchy_border = if hierarchy_focused {
                    Color::Green
                } else {
                    Color::DarkGray
                };
                let config_border = if config_focused {
                    Color::Green
                } else {
                    Color::DarkGray
                };
                let logs_border = if logs_focused {
                    Color::Green
                } else {
                    Color::DarkGray
                };
                let tracing_border = if tracing_focused {
                    Color::Green
                } else {
                    Color::DarkGray
                };

                let hierarchy_widget = Paragraph::new("Task Hierarchy (Loading...)").block(
                    Block::default()
                        .title(" Task Hierarchy ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(hierarchy_border)),
                );
                f.render_widget(hierarchy_widget, top_horizontal[0]);

                let config_widget = Paragraph::new("Task Configuration (Loading...)").block(
                    Block::default()
                        .title(" Task Configuration ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(config_border)),
                );
                f.render_widget(config_widget, right_vertical[0]);

                let logs_widget = Paragraph::new("Task Logs (Loading...)").block(
                    Block::default()
                        .title(" Task Logs ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(logs_border)),
                );
                f.render_widget(logs_widget, right_vertical[1]);

                let tracing_widget = Paragraph::new("System Tracing (Loading...)").block(
                    Block::default()
                        .title(" System Tracing ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(tracing_border)),
                );
                f.render_widget(tracing_widget, main_vertical[1]);

                // Render status bar
                Self::render_status_bar(f, main_vertical[2], &focused_pane);

                // Render help overlay if needed
                if show_help {
                    Self::render_help_overlay(f);
                }
            })?;
        }
        Ok(())
    }

    fn render_status_bar(f: &mut Frame, area: ratatui::layout::Rect, focused_pane: &FocusedPane) {
        let focused_indicator = match focused_pane {
            FocusedPane::TaskHierarchy => "HIERARCHY",
            FocusedPane::TaskConfig => "CONFIG",
            FocusedPane::TaskLogs => "LOGS",
            FocusedPane::Tracing => "TRACING",
        };

        let status = format!(
            " {} | Tab: Switch Panes | q: Quit | h: Help ",
            focused_indicator
        );

        let status_bar = Paragraph::new(status)
            .style(ratatui::style::Style::default().bg(ratatui::style::Color::DarkGray));
        f.render_widget(status_bar, area);
    }

    fn render_help_overlay(f: &mut Frame) {
        let area = f.area();
        let help_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(area)[1];

        let help_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(help_area)[1];

        f.render_widget(Clear, help_area);

        let help_text = "Help\n\n\
            Tab/Shift+Tab: Switch panes\n\
            ↑/↓: Navigate within pane\n\
            Enter: Toggle task expansion\n\
            q: Quit\n\
            h: Toggle this help\n\
            t: Focus tracing pane\n\
            l: Focus logs pane";

        let help_block = Paragraph::new(help_text)
            .block(Block::default().title(" Help ").borders(Borders::ALL))
            .style(ratatui::style::Style::default().bg(ratatui::style::Color::Black));

        f.render_widget(help_block, help_area);
    }

    fn handle_task_event(&self, task_event: TaskEvent) -> io::Result<()> {
        if let Ok(state) = self.state.lock() {
            let registry = state.task_registry.clone();
            drop(state); // Release lock before async operations

            tokio::spawn(async move {
                match task_event {
                    TaskEvent::Started { task_name, .. } => {
                        registry
                            .update_task_state(&task_name, crate::events::TaskState::Running)
                            .await;
                        registry
                            .add_log(
                                &task_name,
                                crate::events::LogStream::System,
                                "Task started".to_string(),
                            )
                            .await;
                    }
                    TaskEvent::Completed {
                        task_name,
                        exit_code,
                        duration_ms,
                    } => {
                        registry
                            .update_task_state(&task_name, crate::events::TaskState::Completed)
                            .await;
                        let log_message = format!(
                            "Task completed (exit code: {}, duration: {}ms)",
                            exit_code, duration_ms
                        );
                        registry
                            .add_log(&task_name, crate::events::LogStream::System, log_message)
                            .await;
                    }
                    TaskEvent::Failed {
                        task_name,
                        error,
                        duration_ms,
                    } => {
                        registry
                            .update_task_state(&task_name, crate::events::TaskState::Failed)
                            .await;
                        let log_message =
                            format!("Task failed: {} (duration: {}ms)", error, duration_ms);
                        registry
                            .add_log(&task_name, crate::events::LogStream::Stderr, log_message)
                            .await;
                    }
                    TaskEvent::Progress { task_name, message } => {
                        registry
                            .add_log(&task_name, crate::events::LogStream::Stdout, message)
                            .await;
                    }
                    TaskEvent::Cancelled { task_name } => {
                        registry
                            .update_task_state(&task_name, crate::events::TaskState::Cancelled)
                            .await;
                        registry
                            .add_log(
                                &task_name,
                                crate::events::LogStream::System,
                                "Task cancelled".to_string(),
                            )
                            .await;
                    }
                    TaskEvent::Log { .. } => {
                        // Log events are already handled
                    }
                }
            });
        }
        Ok(())
    }
}

impl<S> Layer<S> for TuiLayer
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Handle task events
        if let Some(task_event) = extract_task_event(event) {
            if let Err(e) = self.handle_task_event(task_event) {
                eprintln!("TUI error: {e}");
            }
        }

        // Handle general tracing events for the tracing pane
        // For now, we'll skip this until we have proper tracing event conversion
        // TODO: Implement tracing event conversion for the tracing pane
    }
}

impl Drop for TuiLayer {
    fn drop(&mut self) {
        // Signal threads to stop
        if let Ok(state) = self.state.lock() {
            state.running.store(false, Ordering::Relaxed);
        }

        // Clean up terminal
        let _ = disable_raw_mode();
        let _ = io::stdout().execute(LeaveAlternateScreen);
        let _ = io::stdout().execute(DisableMouseCapture);
    }
}

/// Extract task events from tracing events (copied from spinner_layer.rs)
fn extract_task_event(event: &Event<'_>) -> Option<TaskEvent> {
    let mut task_name = None;
    let mut event_type = None;
    let mut message = None;
    let mut error_msg = None;
    let mut duration_ms = None;

    event.record(
        &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| match field.name() {
            "task_name" => task_name = Some(format!("{value:?}").trim_matches('"').to_string()),
            "event_type" => event_type = Some(format!("{value:?}").trim_matches('"').to_string()),
            "message" => message = Some(format!("{value:?}").trim_matches('"').to_string()),
            "error" => error_msg = Some(format!("{value:?}").trim_matches('"').to_string()),
            "duration_ms" => duration_ms = format!("{value:?}").parse().ok(),
            _ => {}
        },
    );

    let task_name = task_name?;
    let timestamp = std::time::Instant::now();

    match event_type?.as_str() {
        "started" => Some(TaskEvent::Started {
            task_name,
            timestamp,
        }),
        "progress" => Some(TaskEvent::Progress {
            task_name,
            message: message.unwrap_or_default(),
        }),
        "completed" => Some(TaskEvent::Completed {
            task_name,
            exit_code: 0,
            duration_ms: duration_ms.unwrap_or(0),
        }),
        "failed" => Some(TaskEvent::Failed {
            task_name,
            error: error_msg.unwrap_or_else(|| "Unknown error".to_string()),
            duration_ms: duration_ms.unwrap_or(0),
        }),
        "cancelled" => Some(TaskEvent::Cancelled { task_name }),
        _ => None,
    }
}
