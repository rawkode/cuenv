use crate::task_executor::TaskExecutor;
use crate::tui::{
    components::{EnvPane, FocusPane, MiniMap},
    event_bus::{EventBus, EventSubscriber},
    events::TaskEvent,
    terminal::{InputEvent, TerminalManager},
};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders},
    Frame,
};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info};

#[derive(Debug, Clone, Copy, PartialEq)]
enum FocusedPane {
    MiniMap,
    TaskDetails,
    Environment,
}

pub struct TuiApp {
    terminal: TerminalManager,
    minimap: MiniMap,
    focus_pane: FocusPane,
    env_pane: EnvPane,
    event_subscriber: EventSubscriber,
    running: bool,
    focused_pane: FocusedPane,
    task_executor: TaskExecutor,
}

impl TuiApp {
    pub async fn new(
        event_bus: EventBus,
        task_executor: TaskExecutor,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let terminal = TerminalManager::new()?;

        let registry = event_bus.registry().clone();
        let minimap = MiniMap::new(registry.clone());
        let focus_pane = FocusPane::new(registry);

        // Start with no environment variables - will be updated when a task is selected
        let env_pane = EnvPane::new(HashMap::new());
        let event_subscriber = event_bus.subscribe();

        Ok(Self {
            terminal,
            minimap,
            focus_pane,
            env_pane,
            event_subscriber,
            running: true,
            focused_pane: FocusedPane::MiniMap,
            task_executor,
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting TUI application");

        // Initial render
        self.minimap.build_tree_lines().await;

        // Initialize env pane with first selected task
        if let Some(task) = self.minimap.get_selected_task() {
            let task_clone = task.clone();
            self.update_env_pane_for_task(&task_clone);
        }

        self.render()?;

        // Timer for updating task info when needed
        let mut update_timer = tokio::time::interval(std::time::Duration::from_millis(50));
        update_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Keep TUI alive until explicit quit, even if tasks error, as per PRD
        while self.running {
            tokio::select! {
                // Handle task events
                Some(event) = self.event_subscriber.recv() => {
                    // If a terminal error occurs, keep UI open; allow user to inspect and press 'q'
                    self.handle_task_event(event).await;
                    self.render()?;
                }

                // Check if we need to update task info
                _ = update_timer.tick() => {
                    if self.focus_pane.needs_task_info_update() {
                        self.focus_pane.update_task_info().await;
                        self.render()?;
                    }
                }

                // Handle input events
                Some(input) = self.terminal.next_event() => {
                    match input {
                        InputEvent::Key(key) => {
                            if TerminalManager::should_quit(&key) {
                                self.running = false;
                            } else {
                                self.handle_key_event(key).await;
                                self.render()?;
                            }
                        }
                        InputEvent::Resize => {
                            self.render()?;
                        }
                        InputEvent::Tick => {
                            // Keep ticking to allow redraw while idle
                        }
                    }
                }

                // No events: small idle sleep to respect CPU budget
                else => {
                    // Yield to avoid busy loop; keep under ~2% of a single core
                    tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                }
            }
        }

        info!("TUI application shutting down");
        Ok(())
    }

    async fn select_task_in_minimap(&mut self, task_name: &str) {
        // Go to top first
        self.minimap.jump_to_top();

        // Search for the task
        let mut found = false;
        for _ in 0..1000 {
            // Safety limit
            if let Some(selected) = self.minimap.get_selected_task() {
                if selected == task_name {
                    found = true;
                    break;
                }
            }
            self.minimap.select_next();
        }

        if found {
            self.focus_pane.set_task(task_name.to_string());
            self.update_env_pane_for_task(task_name);
        }
    }

    fn update_env_pane_for_task(&mut self, task_name: &str) {
        let filtered_vars = self.task_executor.get_task_env_vars(task_name);
        self.env_pane = EnvPane::new(filtered_vars);
    }

    async fn handle_task_event(&mut self, event: TaskEvent) {
        debug!("Handling task event: {:?}", event);

        match &event {
            TaskEvent::Started { task_name, .. } => {
                // Always rebuild tree when a task starts
                self.minimap.build_tree_lines().await;

                // Auto-select the running task to show immediate feedback
                self.select_task_in_minimap(task_name).await;
            }
            TaskEvent::Completed { .. }
            | TaskEvent::Failed { .. }
            | TaskEvent::Cancelled { .. } => {
                // Rebuild tree when task states change
                self.minimap.build_tree_lines().await;

                // On first failure, jump to the first error to reduce time-to-first-error
                if matches!(event, TaskEvent::Failed { .. }) && self.minimap.jump_to_first_error() {
                    if let Some(task) = self.minimap.get_selected_task() {
                        self.focus_pane.set_task(task.clone());
                    }
                }

                // Update focus pane if it's showing the affected task
                if let Some(current_task) = self.focus_pane.get_current_task() {
                    match &event {
                        TaskEvent::Started { task_name, .. }
                        | TaskEvent::Progress { task_name, .. }
                        | TaskEvent::Completed { task_name, .. }
                        | TaskEvent::Failed { task_name, .. }
                        | TaskEvent::Cancelled { task_name } => {
                            if current_task == task_name {
                                self.focus_pane.update_task_info().await;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        // Keep focus pane synced to currently selected node
        if let Some(selected) = self.minimap.get_selected_task() {
            let selected_clone = selected.clone();
            self.focus_pane.set_task(selected_clone.clone());
            self.update_env_pane_for_task(&selected_clone);
        }
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            // Pane switching
            KeyCode::Tab => {
                self.focused_pane = match self.focused_pane {
                    FocusedPane::MiniMap => FocusedPane::TaskDetails,
                    FocusedPane::TaskDetails => FocusedPane::Environment,
                    FocusedPane::Environment => FocusedPane::MiniMap,
                };
            }

            // Navigation
            KeyCode::Up | KeyCode::Char('k') => match self.focused_pane {
                FocusedPane::MiniMap => {
                    self.minimap.select_previous();
                    if let Some(task) = self.minimap.get_selected_task() {
                        let task_clone = task.clone();
                        self.focus_pane.set_task(task_clone.clone());
                        self.update_env_pane_for_task(&task_clone);
                    }
                }
                FocusedPane::TaskDetails => {
                    self.focus_pane.scroll_up(1);
                }
                FocusedPane::Environment => {
                    self.env_pane.select_previous();
                }
            },
            KeyCode::Down | KeyCode::Char('j') => match self.focused_pane {
                FocusedPane::MiniMap => {
                    self.minimap.select_next();
                    if let Some(task) = self.minimap.get_selected_task() {
                        let task_clone = task.clone();
                        self.focus_pane.set_task(task_clone.clone());
                        self.update_env_pane_for_task(&task_clone);
                    }
                }
                FocusedPane::TaskDetails => {
                    self.focus_pane.scroll_down(1);
                }
                FocusedPane::Environment => {
                    self.env_pane.select_next();
                }
            },

            // Tree expansion
            KeyCode::Left | KeyCode::Char('h') => {
                self.minimap.toggle_expand();
                self.minimap.build_tree_lines().await;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.minimap.toggle_expand();
                self.minimap.build_tree_lines().await;
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.minimap.toggle_expand();
                self.minimap.build_tree_lines().await;
            }

            // Scrolling
            KeyCode::PageUp => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus_pane.scroll_up(10);
                } else {
                    self.minimap.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus_pane.scroll_down(10);
                } else {
                    self.minimap.scroll_down(10);
                }
            }

            // Jump commands (PRD: g/G operate on mini-map selection)
            KeyCode::Char('g') => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+g behaves like 'G'
                    self.minimap.jump_to_bottom();
                } else {
                    self.minimap.jump_to_top();
                }
                if let Some(task) = self.minimap.get_selected_task() {
                    let task_clone = task.clone();
                    self.focus_pane.set_task(task_clone.clone());
                    self.update_env_pane_for_task(&task_clone);
                }
            }
            KeyCode::Char('G') => {
                self.minimap.jump_to_bottom();
                if let Some(task) = self.minimap.get_selected_task() {
                    let task_clone = task.clone();
                    self.focus_pane.set_task(task_clone.clone());
                    self.update_env_pane_for_task(&task_clone);
                }
            }
            KeyCode::Char('E') => {
                self.minimap.jump_to_first_error();
                if let Some(task) = self.minimap.get_selected_task() {
                    let task_clone = task.clone();
                    self.focus_pane.set_task(task_clone.clone());
                    self.update_env_pane_for_task(&task_clone);
                }
            }

            // Tree manipulation
            KeyCode::Char('*') => {
                self.minimap.expand_all();
                self.minimap.build_tree_lines().await;
            }
            KeyCode::Char('/') => {
                self.minimap.collapse_all();
                self.minimap.build_tree_lines().await;
            }

            // Focus pane controls
            KeyCode::Char('a') => {
                self.focus_pane.toggle_auto_scroll();
            }

            _ => {}
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let minimap = &mut self.minimap;
        let focus_pane = &mut self.focus_pane;
        let env_pane = &mut self.env_pane;
        let focused = self.focused_pane;

        self.terminal.terminal().draw(|f| {
            Self::draw_ui_static(f, minimap, focus_pane, env_pane, focused);
        })?;
        Ok(())
    }

    fn draw_ui_static(
        frame: &mut Frame<'_>,
        minimap: &mut MiniMap,
        focus_pane: &mut FocusPane,
        env_pane: &mut EnvPane,
        focused: FocusedPane,
    ) {
        // Main layout: split screen horizontally
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Mini-map
                Constraint::Percentage(60), // Right side (focus pane & env)
            ])
            .split(frame.area());

        // Draw mini-map with border highlight if focused
        let minimap_border_style = if focused == FocusedPane::MiniMap {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let minimap_block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(minimap_border_style);
        let minimap_area = minimap_block.inner(chunks[0]);
        frame.render_widget(minimap_block, chunks[0]);
        minimap.render(frame, minimap_area);

        // Split the right side vertically for focus pane and env pane
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60), // Focus pane (task details & logs)
                Constraint::Percentage(40), // Environment pane
            ])
            .split(chunks[1]);

        // Draw focus pane
        focus_pane.render(frame, right_chunks[0]);

        // Draw environment pane with border highlight if focused
        env_pane.render(frame, right_chunks[1]);

        // Draw help bar at the bottom
        let help_text = " Tab: Switch Pane │ ↑↓/jk: Navigate │ ←→/hl/Space: Expand │ E: First Error │ g/G: Top/Bottom │ a: Auto-scroll │ q: Quit ";
        let help_bar = Block::default()
            .title(help_text)
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::TOP);

        let help_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(frame.area())[1];

        frame.render_widget(help_bar, help_area);
    }
}

// Helper function to convert tracing events to TUI events
pub fn tracing_to_tui_event(
    event: &tracing::Event,
    metadata: &tracing::Metadata,
) -> Option<TaskEvent> {
    let mut visitor = EventVisitor::default();
    event.record(&mut visitor);

    match metadata.name() {
        "task_started" => Some(TaskEvent::Started {
            task_name: visitor.task_name?,
            timestamp: Instant::now(),
        }),
        "task_progress" => Some(TaskEvent::Progress {
            task_name: visitor.task_name?,
            message: visitor.message.unwrap_or_default(),
        }),
        "task_completed" => Some(TaskEvent::Completed {
            task_name: visitor.task_name?,
            exit_code: 0,
            duration_ms: visitor.duration_ms.unwrap_or(0),
        }),
        "task_failed" => Some(TaskEvent::Failed {
            task_name: visitor.task_name?,
            error: visitor
                .message
                .unwrap_or_else(|| "Unknown error".to_string()),
            duration_ms: visitor.duration_ms.unwrap_or(0),
        }),
        _ => None,
    }
}

#[derive(Default)]
struct EventVisitor {
    task_name: Option<String>,
    message: Option<String>,
    duration_ms: Option<u64>,
}

impl tracing::field::Visit for EventVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "task_name" => self.task_name = Some(value.to_string()),
            "message" => self.message = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "duration_ms" {
            self.duration_ms = Some(value)
        }
    }

    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {
        // Handle debug fields if needed
    }
}
