//! Bottom-style TUI application
//! Main TUI application based on bottom's event-driven architecture

use crate::{
    app::focus::FocusedPane,
    components::{TaskConfigPane, TaskHierarchy, TaskLogsPane, TracingPane},
    events::{ControlEvent, FlattenedTask, TaskEvent, TaskRegistry, TuiEvent},
    terminal::{setup_panic_hook, TerminalManager},
};
use crossterm::event::{
    self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, MouseEvent,
};
use cuenv_core::TaskDefinition;
use cuenv_task::executor::TaskExecutor;
use ratatui::Frame;
use std::collections::HashMap;
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Configuration for the TUI application
pub struct TuiConfig {
    pub target_task: Option<String>,
    pub auto_execute: bool,
}

/// Main TUI application following bottom's architecture
pub struct TuiApp {
    // Terminal management
    terminal_manager: TerminalManager,

    // Components
    task_hierarchy: TaskHierarchy,
    task_config_pane: TaskConfigPane,
    task_logs_pane: TaskLogsPane,
    tracing_pane: TracingPane,

    // Application state
    running: bool,
    focused_pane: FocusedPane,
    show_help: bool,

    // Task execution
    task_executor: TaskExecutor,
    task_registry: Arc<TaskRegistry>,
    config: TuiConfig,

    // Event handling
    event_receiver: Receiver<TuiEvent>,
    control_sender: Sender<ControlEvent>,

    // Threads
    _input_thread: JoinHandle<()>,
    _task_thread: Option<JoinHandle<()>>,
}

impl TuiApp {
    /// Create a new TUI application for task execution
    pub async fn new_with_task(
        task_executor: TaskExecutor,
        target_task: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config = TuiConfig {
            target_task: Some(target_task.to_string()),
            auto_execute: true,
        };

        Self::new_internal(task_executor, config).await
    }

    /// Create a new TUI application for task listing/selection
    pub async fn new_for_listing(
        task_executor: TaskExecutor,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config = TuiConfig {
            target_task: None,
            auto_execute: false,
        };

        Self::new_internal(task_executor, config).await
    }

    /// Internal constructor
    async fn new_internal(
        task_executor: TaskExecutor,
        config: TuiConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Set up panic hook first
        setup_panic_hook();

        // Create terminal manager
        let terminal_manager = TerminalManager::new()?;

        // Create task registry
        let task_registry = Arc::new(TaskRegistry::new());

        // Build execution plan if we have a target task
        if let Some(ref target_task) = config.target_task {
            if let Ok(dag) = task_executor.build_dag(&[target_task.clone()]) {
                // Register all tasks from the DAG to ensure they show up in TUI
                let flattened_tasks = dag.get_flattened_tasks();
                for flattened_task in flattened_tasks {
                    // Skip barrier tasks (they contain "__" in their names)
                    if flattened_task.id.contains("__") {
                        continue;
                    }

                    // Get task definition to extract dependencies
                    let dependencies =
                        if let Some(task_def) = dag.get_task_definition(&flattened_task.id) {
                            task_def
                                .dependencies
                                .iter()
                                .map(|dep| dep.name.clone())
                                .collect()
                        } else {
                            Vec::new()
                        };

                    task_registry
                        .register_task(flattened_task.id.clone(), dependencies)
                        .await;
                }
            }
        } else {
            // For listing mode, register all available tasks
            let available_tasks = task_executor.list_tasks();
            for (task_name, _description) in &available_tasks {
                task_registry
                    .register_task(task_name.clone(), Vec::new())
                    .await;
            }
        }

        // Create components
        let task_hierarchy = TaskHierarchy::new((*task_registry).clone());
        let task_config_pane = TaskConfigPane::new();
        let task_logs_pane = TaskLogsPane::new();
        let tracing_pane = TracingPane::new();

        // Create event channels
        let (event_sender, event_receiver) = mpsc::channel();
        let (control_sender, control_receiver) = mpsc::channel();

        // Create input thread
        let input_thread = Self::create_input_thread(event_sender.clone());

        // Create task execution thread if needed
        let task_thread = if config.target_task.is_some() {
            Some(Self::create_task_thread(
                event_sender.clone(),
                control_receiver,
                task_executor.clone(),
                config.target_task.clone().unwrap(),
            ))
        } else {
            None
        };

        Ok(Self {
            terminal_manager,
            task_hierarchy,
            task_config_pane,
            task_logs_pane,
            tracing_pane,
            running: true,
            focused_pane: FocusedPane::TaskHierarchy,
            show_help: false,
            task_executor,
            task_registry,
            config,
            event_receiver,
            control_sender,
            _input_thread: input_thread,
            _task_thread: task_thread,
        })
    }

    /// Main event loop - this blocks until the application exits
    pub async fn run(&mut self) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        // Initial setup
        self.task_hierarchy.build_tree_lines().await;
        if let Some(task_name) = self.task_hierarchy.get_selected_task().cloned() {
            self.update_panes_for_task(&task_name).await;
        }

        // Initial render
        self.render()?;

        // Start task execution if auto-execute is enabled
        if self.config.auto_execute {
            if let Some(ref target_task) = self.config.target_task {
                tracing::info!("TUI: Auto-executing task: {}", target_task);
                let _ = self.control_sender.send(ControlEvent::Resume);
            }
        }

        let mut last_render = Instant::now();
        let exit_code = 0;

        // Main event loop
        while self.running {
            // Handle events with timeout
            match self.event_receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    match event {
                        TuiEvent::Terminate => {
                            self.running = false;
                        }
                        TuiEvent::Resize => {
                            self.render()?;
                        }
                        TuiEvent::KeyInput(key_event) => {
                            if self.handle_key_event(key_event).await {
                                self.running = false;
                            }
                            self.render()?;
                        }
                        TuiEvent::MouseInput(mouse_event) => {
                            self.handle_mouse_event(mouse_event);
                            self.render()?;
                        }
                        TuiEvent::TaskUpdate(task_event) => {
                            self.handle_task_event(task_event).await;

                            // Throttled render for task updates
                            if last_render.elapsed() >= Duration::from_millis(100) {
                                self.render()?;
                                last_render = Instant::now();
                            }
                        }
                        TuiEvent::TracingUpdate(tracing_event) => {
                            self.tracing_pane.add_event(tracing_event);

                            // Throttled render for tracing updates
                            if last_render.elapsed() >= Duration::from_millis(100) {
                                self.render()?;
                                last_render = Instant::now();
                            }
                        }
                        TuiEvent::DAGReceived {
                            tasks,
                            execution_levels: _,
                            task_definitions,
                        } => {
                            self.handle_dag_received(tasks, task_definitions).await;
                            self.render()?;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Periodic updates
                    if last_render.elapsed() >= Duration::from_millis(500) {
                        self.task_hierarchy.build_tree_lines().await;
                        if let Some(task_name) = self.task_hierarchy.get_selected_task().cloned() {
                            self.update_panes_for_task(&task_name).await;
                        }
                        self.render()?;
                        last_render = Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.running = false;
                }
            }
        }

        Ok(exit_code)
    }

    /// Handle keyboard events
    async fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        // Handle global keys first
        match key_event.code {
            KeyCode::Char('q') if !self.show_help => return true,
            KeyCode::Char('c')
                if key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                return true
            }
            KeyCode::Char('h') => {
                self.show_help = !self.show_help;
                return false;
            }
            KeyCode::Tab => {
                self.focused_pane = self.focused_pane.next();
                return false;
            }
            KeyCode::BackTab => {
                self.focused_pane = self.focused_pane.previous();
                return false;
            }
            KeyCode::Char('t') => {
                self.focused_pane = FocusedPane::TracingOutput;
                return false;
            }
            KeyCode::Char('l') => {
                self.focused_pane = FocusedPane::TaskLogs;
                return false;
            }
            _ => {}
        }

        // Handle pane-specific keys
        match self.focused_pane {
            FocusedPane::TaskHierarchy => {
                match key_event.code {
                    KeyCode::Up => {
                        self.task_hierarchy.move_up();
                        if let Some(task_name) = self.task_hierarchy.get_selected_task().cloned() {
                            self.update_panes_for_task(&task_name).await;
                        }
                    }
                    KeyCode::Down => {
                        self.task_hierarchy.move_down();
                        if let Some(task_name) = self.task_hierarchy.get_selected_task().cloned() {
                            self.update_panes_for_task(&task_name).await;
                        }
                    }
                    KeyCode::Enter => {
                        self.task_hierarchy.toggle_selected();
                    }
                    KeyCode::Char('r') | KeyCode::Char('x') => {
                        if !self.config.auto_execute {
                            if let Some(task_name) = self.task_hierarchy.get_selected_task() {
                                tracing::info!(
                                    "TUI: Manual execution requested for task: {}",
                                    task_name
                                );
                                // TODO: Implement manual task execution
                            }
                        }
                    }
                    _ => {}
                }
            }
            FocusedPane::TaskLogs => match key_event.code {
                KeyCode::Up => self.task_logs_pane.scroll_up(1),
                KeyCode::Down => self.task_logs_pane.scroll_down(1),
                KeyCode::PageUp => self.task_logs_pane.scroll_up(10),
                KeyCode::PageDown => self.task_logs_pane.scroll_down(10),
                _ => {}
            },
            FocusedPane::TaskConfig => match key_event.code {
                KeyCode::Up => self.task_config_pane.scroll_up(1),
                KeyCode::Down => self.task_config_pane.scroll_down(1),
                _ => {}
            },
            FocusedPane::TracingOutput => match key_event.code {
                KeyCode::Up => self.tracing_pane.scroll_up(1),
                KeyCode::Down => self.tracing_pane.scroll_down(1),
                KeyCode::PageUp => self.tracing_pane.scroll_up(10),
                KeyCode::PageDown => self.tracing_pane.scroll_down(10),
                KeyCode::Char('c') => self.tracing_pane.clear_events(),
                _ => {}
            },
        }

        false
    }

    /// Handle mouse events
    fn handle_mouse_event(&mut self, _mouse_event: MouseEvent) {
        // TODO: Implement mouse handling
    }

    /// Handle task events from the execution system
    async fn handle_task_event(&mut self, task_event: TaskEvent) {
        match &task_event {
            TaskEvent::Started { task_name, .. } => {
                self.task_registry
                    .update_task_state(task_name, crate::events::TaskState::Running)
                    .await;
            }
            TaskEvent::Completed { task_name, .. } => {
                self.task_registry
                    .update_task_state(task_name, crate::events::TaskState::Completed)
                    .await;
            }
            TaskEvent::Failed { task_name, .. } => {
                self.task_registry
                    .update_task_state(task_name, crate::events::TaskState::Failed)
                    .await;
            }
            TaskEvent::Cancelled { task_name } => {
                self.task_registry
                    .update_task_state(task_name, crate::events::TaskState::Cancelled)
                    .await;
            }
            TaskEvent::Log {
                task_name,
                stream,
                content,
            } => {
                self.task_registry
                    .add_log(task_name, stream.clone(), content.clone())
                    .await;
            }
            _ => {}
        }

        // Update the currently selected task panes if it matches this event
        if let Some(selected_task) = self.task_hierarchy.get_selected_task() {
            if let TaskEvent::Started { task_name, .. }
            | TaskEvent::Completed { task_name, .. }
            | TaskEvent::Failed { task_name, .. }
            | TaskEvent::Log { task_name, .. } = &task_event
            {
                if selected_task == task_name {
                    self.update_panes_for_task(task_name).await;
                }
            }
        }
    }

    /// Update config and logs panes for the selected task
    async fn update_panes_for_task(&mut self, task_name: &str) {
        if let Some(task_info) = self.task_registry.get_task(task_name).await {
            self.task_config_pane
                .set_task(task_name.to_string(), Some(task_info.clone()));
            self.task_logs_pane
                .set_task_with_registry(task_name.to_string(), Some(task_info), &self.task_registry)
                .await;
        }
    }

    /// Handle DAG received event to build task hierarchy
    async fn handle_dag_received(
        &mut self,
        tasks: Vec<FlattenedTask>,
        _task_definitions: HashMap<String, TaskDefinition>,
    ) {
        // Clear existing tasks
        self.task_registry = Arc::new(TaskRegistry::new());

        // Register all tasks from the DAG with proper structure
        for flattened_task in &tasks {
            // Skip barrier tasks
            if flattened_task.id.contains("__") {
                continue;
            }

            // Use dependencies directly from the DAG
            let dependencies = flattened_task.dependencies.clone();

            // Register the task with its full ID and dependencies
            self.task_registry
                .register_task(flattened_task.id.clone(), dependencies)
                .await;
        }

        // Rebuild the task hierarchy from the new DAG structure
        self.task_hierarchy.build_tree_lines().await;

        // If no task is selected yet, select the first one
        if self.task_hierarchy.get_selected_task().is_none() {
            if let Some(first_task) = tasks.first() {
                self.task_hierarchy.selected_task = Some(first_task.id.clone());
                self.update_panes_for_task(&first_task.id).await;
            }
        }
    }

    /// Check if all tasks are complete
    async fn all_tasks_complete(&self) -> bool {
        let all_tasks = self.task_registry.get_all_tasks().await;
        for (_, task_info) in all_tasks {
            match task_info.state {
                crate::events::TaskState::Queued | crate::events::TaskState::Running => {
                    return false
                }
                _ => {}
            }
        }
        true
    }

    /// Get the final exit code based on task results
    async fn get_final_exit_code(&self) -> i32 {
        let all_tasks = self.task_registry.get_all_tasks().await;
        for (_, task_info) in all_tasks {
            if matches!(task_info.state, crate::events::TaskState::Failed) {
                return task_info.exit_code.unwrap_or(1);
            }
        }
        0
    }

    /// Render the TUI
    fn render(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let show_help = self.show_help;
        let focused_pane = self.focused_pane;

        self.terminal_manager.terminal_mut().draw(|f| {
            let size = f.area();

            if show_help {
                render_help_overlay(f);
                return;
            }

            // Create 4-pane layout that scales properly with terminal size
            let main_vertical = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Percentage(70), // Top content (main panes)
                    ratatui::layout::Constraint::Percentage(25), // Tracing pane
                    ratatui::layout::Constraint::Min(1),         // Status bar (minimum 1 line)
                ])
                .split(size);

            let top_horizontal = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Percentage(40), // Task Hierarchy
                    ratatui::layout::Constraint::Percentage(60), // Config + Logs
                ])
                .split(main_vertical[0]);

            let right_vertical = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Percentage(60), // Task Config
                    ratatui::layout::Constraint::Percentage(40), // Task Logs
                ])
                .split(top_horizontal[1]);

            // Render actual components
            self.task_hierarchy.render_with_focus(
                f,
                top_horizontal[0],
                matches!(focused_pane, FocusedPane::TaskHierarchy),
            );
            self.task_config_pane.render_with_focus(
                f,
                right_vertical[0],
                matches!(focused_pane, FocusedPane::TaskConfig),
            );
            self.task_logs_pane.render_with_focus(
                f,
                right_vertical[1],
                matches!(focused_pane, FocusedPane::TaskLogs),
            );
            self.tracing_pane.render_with_focus(
                f,
                main_vertical[1],
                matches!(focused_pane, FocusedPane::TracingOutput),
            );

            // Render status bar
            render_status_bar(f, main_vertical[2], focused_pane);
        })?;
        Ok(())
    }

    /// Render a single frame
    fn render_frame(&mut self, f: &mut Frame) {
        let size = f.area();

        if self.show_help {
            self.render_help_overlay(f);
            return;
        }

        // Create 4-pane layout - ensure minimum sizes
        let main_vertical = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Min(10), // Top content (minimum height)
                ratatui::layout::Constraint::Length(8), // Tracing pane
                ratatui::layout::Constraint::Length(1), // Status bar
            ])
            .split(size);

        let top_horizontal = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(40), // Task Hierarchy
                ratatui::layout::Constraint::Percentage(60), // Config + Logs
            ])
            .split(main_vertical[0]);

        let right_vertical = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Percentage(60), // Task Config
                ratatui::layout::Constraint::Percentage(40), // Task Logs
            ])
            .split(top_horizontal[1]);

        // Render components
        self.task_hierarchy.render_with_focus(
            f,
            top_horizontal[0],
            matches!(self.focused_pane, FocusedPane::TaskHierarchy),
        );

        self.task_config_pane.render_with_focus(
            f,
            right_vertical[0],
            matches!(self.focused_pane, FocusedPane::TaskConfig),
        );

        self.task_logs_pane.render_with_focus(
            f,
            right_vertical[1],
            matches!(self.focused_pane, FocusedPane::TaskLogs),
        );

        self.tracing_pane.render_with_focus(
            f,
            main_vertical[1],
            matches!(self.focused_pane, FocusedPane::TracingOutput),
        );

        // Render status bar
        self.render_status_bar(f, main_vertical[2]);
    }

    /// Render the status bar
    fn render_status_bar(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let focused_indicator = match self.focused_pane {
            FocusedPane::TaskHierarchy => "HIERARCHY",
            FocusedPane::TaskConfig => "CONFIG",
            FocusedPane::TaskLogs => "LOGS",
            FocusedPane::TracingOutput => "TRACING",
        };

        let status = format!(
            " {} | Tab: Switch Panes | q: Quit | h: Help ",
            focused_indicator
        );

        let status_bar = ratatui::widgets::Paragraph::new(status)
            .style(ratatui::style::Style::default().bg(ratatui::style::Color::DarkGray));
        f.render_widget(status_bar, area);
    }

    /// Render help overlay
    fn render_help_overlay(&self, f: &mut Frame) {
        let area = f.area();
        let help_area = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(25),
                ratatui::layout::Constraint::Percentage(50),
                ratatui::layout::Constraint::Percentage(25),
            ])
            .split(area)[1];

        let help_area = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Percentage(25),
                ratatui::layout::Constraint::Percentage(50),
                ratatui::layout::Constraint::Percentage(25),
            ])
            .split(help_area)[1];

        f.render_widget(ratatui::widgets::Clear, help_area);

        let help_text = "Help\n\n\
            Tab/Shift+Tab: Switch panes\n\
            ↑/↓: Navigate within pane\n\
            Enter: Toggle task expansion\n\
            r/x: Execute selected task\n\
            q: Quit\n\
            h: Toggle this help\n\
            t: Focus tracing pane\n\
            l: Focus logs pane\n\
            c: Clear tracing (in tracing pane)";

        let help_block = ratatui::widgets::Paragraph::new(help_text)
            .block(
                ratatui::widgets::Block::default()
                    .title(" Help ")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .style(ratatui::style::Style::default().bg(ratatui::style::Color::Black));

        f.render_widget(help_block, help_area);
    }

    /// Create input handling thread
    fn create_input_thread(sender: Sender<TuiEvent>) -> JoinHandle<()> {
        thread::spawn(move || loop {
            if let Ok(poll_result) = event::poll(Duration::from_millis(100)) {
                if poll_result {
                    if let Ok(event) = event::read() {
                        match event {
                            CrosstermEvent::Resize(_, _) => {
                                if sender.send(TuiEvent::Resize).is_err() {
                                    break;
                                }
                            }
                            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                                if sender.send(TuiEvent::KeyInput(key)).is_err() {
                                    break;
                                }
                            }
                            CrosstermEvent::Mouse(mouse) => {
                                if sender.send(TuiEvent::MouseInput(mouse)).is_err() {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            } else {
                break;
            }
        })
    }

    /// Create task execution thread
    fn create_task_thread(
        sender: Sender<TuiEvent>,
        _control_receiver: Receiver<ControlEvent>,
        task_executor: TaskExecutor,
        target_task: String,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            // Create a Tokio runtime for the task execution
            let rt = Runtime::new().unwrap();

            rt.block_on(async {
                // Set up a TUI tracing layer that routes events to the TUI
                use tracing::{Event, Subscriber};
                use tracing_subscriber::layer::Context;
                use tracing_subscriber::{layer::SubscriberExt, Layer, Registry};

                // Custom layer that routes tracing events to TUI via sender
                struct TaskExecutionTracingLayer {
                    sender: Sender<TuiEvent>,
                }

                impl<S> Layer<S> for TaskExecutionTracingLayer
                where
                    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
                {
                    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
                        use crate::app::tracing::tracing_to_tui_event;
                        use crate::events::{TracingEvent, TracingLevel, TuiEvent};
                        use chrono::Local;

                        // Try to convert to task event first
                        if let Some(task_event) = tracing_to_tui_event(event, event.metadata()) {
                            let _ = self.sender.send(TuiEvent::TaskUpdate(task_event));
                        }

                        // Always send as tracing event for general output
                        let mut visitor = crate::app::tracing::EventVisitor::default();
                        event.record(&mut visitor);

                        let level = match *event.metadata().level() {
                            tracing::Level::TRACE => TracingLevel::Trace,
                            tracing::Level::DEBUG => TracingLevel::Debug,
                            tracing::Level::INFO => TracingLevel::Info,
                            tracing::Level::WARN => TracingLevel::Warn,
                            tracing::Level::ERROR => TracingLevel::Error,
                        };

                        let tracing_event = TracingEvent {
                            timestamp: Local::now(),
                            level,
                            target: event.metadata().target().to_string(),
                            message: visitor.message.unwrap_or_else(|| "".to_string()),
                            fields: visitor.fields.into_iter().collect(),
                        };

                        let _ = self.sender.send(TuiEvent::TracingUpdate(tracing_event));
                    }
                }

                let tui_layer = TaskExecutionTracingLayer {
                    sender: sender.clone(),
                };

                let subscriber = Registry::default().with(tui_layer);
                let _guard = tracing::subscriber::set_default(subscriber);

                // Build and send DAG to TUI before execution starts
                if let Ok(dag) = task_executor.build_dag(&[target_task.clone()]) {
                    // Convert from executor FlattenedTask to TUI FlattenedTask
                    let tui_tasks: Vec<FlattenedTask> = dag
                        .get_flattened_tasks()
                        .iter()
                        .map(|task| FlattenedTask {
                            id: task.id.clone(),
                            name: task.name.clone(),
                            group_path: task.group_path.clone(),
                            dependencies: task.dependencies.clone(),
                            is_barrier: task.is_barrier,
                        })
                        .collect();

                    let _ = sender.send(TuiEvent::DAGReceived {
                        tasks: tui_tasks,
                        execution_levels: dag.get_execution_levels().unwrap_or_default(),
                        task_definitions: dag.get_task_definitions().clone(),
                    });
                }

                // Set up tracing layer to route task events to TUI

                let tui_channel_layer = TuiChannelLayer::new(sender.clone());

                // Execute the task with tracing subscriber to route events to TUI
                let result = {
                    let subscriber = Registry::default().with(tui_channel_layer);

                    // Use with_default to ensure the subscriber is active for the entire task execution
                    tracing::subscriber::with_default(subscriber, || async {
                        task_executor
                            .execute_tasks(&[target_task.clone()], &[], false, true)
                            .await
                    })
                    .await
                };

                // Send completion event directly to TUI
                match result {
                    Ok(exit_code) if exit_code == 0 => {
                        let _ = sender.send(TuiEvent::TaskUpdate(TaskEvent::Completed {
                            task_name: target_task.clone(),
                            exit_code,
                            duration_ms: 0, // TODO: Track actual duration
                        }));
                    }
                    Ok(exit_code) => {
                        let _ = sender.send(TuiEvent::TaskUpdate(TaskEvent::Failed {
                            task_name: target_task.clone(),
                            error: format!("Task failed with exit code: {}", exit_code),
                            duration_ms: 0, // TODO: Track actual duration
                        }));
                    }
                    Err(e) => {
                        let _ = sender.send(TuiEvent::TaskUpdate(TaskEvent::Failed {
                            task_name: target_task.clone(),
                            error: format!("Task execution error: {}", e),
                            duration_ms: 0, // TODO: Track actual duration
                        }));
                    }
                }
            });
        })
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        // Terminal cleanup is handled by TerminalManager's Drop implementation
    }
}

/// Tracing layer that routes task events to TUI via channel
struct TuiChannelLayer {
    sender: Sender<TuiEvent>,
}

impl TuiChannelLayer {
    fn new(sender: Sender<TuiEvent>) -> Self {
        Self { sender }
    }
}

impl<S> tracing_subscriber::Layer<S> for TuiChannelLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        use crate::app::tracing::tracing_to_tui_event;

        if let Some(task_event) = tracing_to_tui_event(event, event.metadata()) {
            let _ = self.sender.send(TuiEvent::TaskUpdate(task_event));
        }
    }
}

/// Render a placeholder component
fn render_placeholder(f: &mut Frame, area: ratatui::layout::Rect, title: &str, focused: bool) {
    use ratatui::{
        style::{Color, Style},
        widgets::{Block, Borders, Paragraph},
    };

    let border_style = if focused {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let content = format!("{} (Loading...)", title);
    let widget = Paragraph::new(content).block(
        Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    f.render_widget(widget, area);
}

/// Render the status bar
fn render_status_bar(f: &mut Frame, area: ratatui::layout::Rect, focused_pane: FocusedPane) {
    use ratatui::{
        style::{Color, Style},
        widgets::Paragraph,
    };

    let focused_indicator = match focused_pane {
        FocusedPane::TaskHierarchy => "HIERARCHY",
        FocusedPane::TaskConfig => "CONFIG",
        FocusedPane::TaskLogs => "LOGS",
        FocusedPane::TracingOutput => "TRACING",
    };

    let status = format!(
        " {} | Tab: Switch Panes | q: Quit | h: Help ",
        focused_indicator
    );

    let status_bar = Paragraph::new(status).style(Style::default().bg(Color::DarkGray));
    f.render_widget(status_bar, area);
}

/// Render help overlay
fn render_help_overlay(f: &mut Frame) {
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        style::{Color, Style},
        widgets::{Block, Borders, Clear, Paragraph},
    };

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
        r/x: Execute selected task\n\
        q: Quit\n\
        h: Toggle this help\n\
        t: Focus tracing pane\n\
        l: Focus logs pane\n\
        c: Clear tracing (in tracing pane)";

    let help_block = Paragraph::new(help_text)
        .block(Block::default().title(" Help ").borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));

    f.render_widget(help_block, help_area);
}
