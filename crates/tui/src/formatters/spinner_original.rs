use crate::events::{TaskEvent, TaskRegistry, TaskState};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    style::{Attribute, Color, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use cuenv_task::TaskExecutionPlan;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Spinner animation frames
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Progress bar characters
const PROGRESS_FULL: &str = "█";
const PROGRESS_EMPTY: &str = "░";

/// Task display state
#[derive(Clone, Debug)]
struct TaskDisplay {
    name: String,
    state: TaskState,
    message: Option<String>,
    progress: Option<f32>,
    depth: usize,
    dependencies: Vec<String>,
    start_time: Option<Instant>,
    end_time: Option<Instant>,
    line_number: Option<u16>,
    spinner_frame: usize,
    is_skipped: bool,
    skip_reason: Option<String>,
}

impl TaskDisplay {
    fn new(name: String, depth: usize, dependencies: Vec<String>) -> Self {
        Self {
            name,
            state: TaskState::Queued,
            message: None,
            progress: None,
            depth,
            dependencies,
            start_time: None,
            end_time: None,
            line_number: None,
            spinner_frame: 0,
            is_skipped: false,
            skip_reason: None,
        }
    }

    fn duration_str(&self) -> String {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => {
                let duration = end.duration_since(start);
                format!("{:.1}s", duration.as_secs_f32())
            }
            (Some(start), None) => {
                let duration = Instant::now().duration_since(start);
                format!("{:.1}s", duration.as_secs_f32())
            }
            _ => "0.0s".to_string(),
        }
    }

    fn status_icon(&self) -> &'static str {
        if self.is_skipped {
            "✔"
        } else {
            match self.state {
                TaskState::Queued => "◌",
                TaskState::Running => SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()],
                TaskState::Completed => "✔",
                TaskState::Failed => "✖",
                TaskState::Cancelled => "⊘",
            }
        }
    }

    fn status_color(&self) -> Color {
        if self.is_skipped {
            Color::Yellow
        } else {
            match self.state {
                TaskState::Queued => Color::DarkGrey,
                TaskState::Running => Color::Blue,
                TaskState::Completed => Color::Green,
                TaskState::Failed => Color::Red,
                TaskState::Cancelled => Color::DarkRed,
            }
        }
    }

    fn format_progress_bar(&self, width: usize) -> String {
        if let Some(progress) = self.progress {
            let filled = ((progress / 100.0) * width as f32) as usize;
            let empty = width.saturating_sub(filled);
            format!(
                "[{}{}]",
                PROGRESS_FULL.repeat(filled),
                PROGRESS_EMPTY.repeat(empty)
            )
        } else if self.state == TaskState::Running {
            // Show indeterminate progress spinner
            let pos = self.spinner_frame % (width * 2);
            let mut bar = vec![PROGRESS_EMPTY; width];

            // Create a wave effect
            for i in 0..3 {
                let idx = (pos + i) % width;
                if idx < width {
                    bar[idx] = PROGRESS_FULL;
                }
            }

            format!("[{}]", bar.join(""))
        } else {
            String::new()
        }
    }
}

/// Docker Compose-style formatter with hierarchy display
pub struct SpinnerFormatter {
    tasks: Arc<RwLock<HashMap<String, TaskDisplay>>>,
    task_order: Vec<String>,
    start_line: u16,
    total_tasks: usize,
    completed_tasks: Arc<RwLock<usize>>,
    failed_tasks: Arc<RwLock<usize>>,
    _task_registry: TaskRegistry,
}

impl SpinnerFormatter {
    pub fn new(task_registry: TaskRegistry) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            task_order: Vec::new(),
            start_line: 0,
            total_tasks: 0,
            completed_tasks: Arc::new(RwLock::new(0)),
            failed_tasks: Arc::new(RwLock::new(0)),
            _task_registry: task_registry,
        }
    }

    /// Initialize the formatter with the execution plan
    pub async fn initialize(&mut self, plan: &TaskExecutionPlan) -> io::Result<()> {
        self.total_tasks = plan.tasks.len();

        // Build task hierarchy and determine display order
        let mut task_depths = HashMap::new();
        let mut task_order = Vec::new();

        // Calculate depth for each task based on dependency levels
        for (level_idx, level_tasks) in plan.levels.iter().enumerate() {
            for task_name in level_tasks {
                task_depths.insert(task_name.clone(), level_idx);
            }
        }

        // Create display order that respects hierarchy
        Self::build_display_order(plan, &mut task_order);
        self.task_order = task_order;

        // Initialize task displays
        let mut tasks = HashMap::new();
        for (idx, task_name) in self.task_order.iter().enumerate() {
            let depth = *task_depths.get(task_name).unwrap_or(&0);
            let deps = plan
                .tasks
                .get(task_name)
                .map(|t| t.dependency_names())
                .unwrap_or_default();

            let mut display = TaskDisplay::new(task_name.clone(), depth, deps);
            display.line_number = Some(self.start_line + idx as u16 + 2);
            tasks.insert(task_name.clone(), display);
        }

        *self.tasks.write().await = tasks;

        // Clear screen and hide cursor
        let mut stdout = io::stdout();
        stdout.execute(Hide)?;
        stdout.execute(Clear(ClearType::FromCursorDown))?;

        // Draw initial state
        self.draw_all().await?;

        Ok(())
    }

    /// Build display order that groups tasks by their dependencies
    fn build_display_order(plan: &TaskExecutionPlan, order: &mut Vec<String>) {
        // Process tasks level by level
        let mut processed = std::collections::HashSet::new();

        for level_tasks in &plan.levels {
            for task_name in level_tasks {
                if !processed.contains(task_name) {
                    Self::add_task_and_dependents(task_name, plan, order, &mut processed);
                }
            }
        }
    }

    /// Recursively add a task and its dependents to the display order
    fn add_task_and_dependents(
        task_name: &str,
        plan: &TaskExecutionPlan,
        order: &mut Vec<String>,
        processed: &mut std::collections::HashSet<String>,
    ) {
        if processed.contains(task_name) {
            return;
        }

        order.push(task_name.to_string());
        processed.insert(task_name.to_string());

        // Find tasks that depend on this one
        for (other_name, other_config) in &plan.tasks {
            let deps = other_config.dependency_names();
            if deps.contains(&task_name.to_string()) && !processed.contains(other_name) {
                // This task depends on the current one, add it next (with indentation)
                Self::add_task_and_dependents(other_name, plan, order, processed);
            }
        }
    }

    /// Draw all tasks
    async fn draw_all(&self) -> io::Result<()> {
        let mut stdout = io::stdout();

        // Draw header
        stdout.execute(MoveTo(0, self.start_line))?;
        stdout.execute(Clear(ClearType::CurrentLine))?;

        let completed = *self.completed_tasks.read().await;
        let failed = *self.failed_tasks.read().await;

        stdout.execute(SetAttribute(Attribute::Bold))?;
        if failed > 0 {
            stdout.execute(SetForegroundColor(Color::Red))?;
            write!(stdout, "[✖] ")?;
        } else if completed == self.total_tasks {
            stdout.execute(SetForegroundColor(Color::Green))?;
            write!(stdout, "[✔] ")?;
        } else {
            stdout.execute(SetForegroundColor(Color::Blue))?;
            write!(stdout, "[+] ")?;
        }

        write!(stdout, "Running {}/{}", completed, self.total_tasks)?;
        if failed > 0 {
            write!(stdout, " ({failed} failed)")?;
        }
        stdout.execute(ResetColor)?;
        stdout.execute(SetAttribute(Attribute::Reset))?;
        writeln!(stdout)?;

        // Draw each task
        let tasks = self.tasks.read().await;
        for task_name in &self.task_order {
            if let Some(task) = tasks.get(task_name) {
                self.draw_task(&mut stdout, task)?;
            }
        }

        stdout.flush()?;
        Ok(())
    }

    /// Draw a single task line
    fn draw_task(&self, stdout: &mut io::Stdout, task: &TaskDisplay) -> io::Result<()> {
        if let Some(line) = task.line_number {
            stdout.execute(MoveTo(0, line))?;
            stdout.execute(Clear(ClearType::CurrentLine))?;

            // Indentation based on depth
            let indent = " ".repeat(task.depth * 2);
            write!(stdout, "{indent}")?;

            // Status icon
            stdout.execute(SetForegroundColor(task.status_color()))?;
            write!(stdout, "{} ", task.status_icon())?;

            // Task name
            stdout.execute(SetAttribute(Attribute::Bold))?;
            write!(stdout, "{:<20}", task.name)?;
            stdout.execute(SetAttribute(Attribute::Reset))?;

            // Progress bar or status message
            if task.state == TaskState::Running {
                stdout.execute(SetForegroundColor(Color::Blue))?;
                write!(stdout, " {} ", task.format_progress_bar(10))?;
                write!(stdout, "Running")?;
            } else if task.is_skipped {
                stdout.execute(SetForegroundColor(Color::Yellow))?;
                let reason = task.skip_reason.as_deref().unwrap_or("Already cached");
                write!(stdout, " Skipped - {reason}")?;
            } else if task.state == TaskState::Completed {
                stdout.execute(SetForegroundColor(Color::Green))?;
                write!(stdout, " Completed")?;
            } else if task.state == TaskState::Failed {
                stdout.execute(SetForegroundColor(Color::Red))?;
                write!(stdout, " Failed")?;
                if let Some(msg) = &task.message {
                    write!(stdout, " - {msg}")?;
                }
            } else if task.state == TaskState::Queued {
                stdout.execute(SetForegroundColor(Color::DarkGrey))?;
                if !task.dependencies.is_empty() {
                    write!(stdout, " Waiting for dependencies")?;
                } else {
                    write!(stdout, " Queued")?;
                }
            }

            // Duration
            if task.state != TaskState::Queued {
                stdout.execute(SetForegroundColor(Color::DarkGrey))?;
                write!(stdout, " {}", task.duration_str())?;
            }

            stdout.execute(ResetColor)?;
        }

        Ok(())
    }

    /// Handle a task event
    pub async fn handle_event(&self, event: TaskEvent) -> io::Result<()> {
        let mut tasks = self.tasks.write().await;

        match event {
            TaskEvent::Started {
                task_name,
                timestamp,
            } => {
                if let Some(task) = tasks.get_mut(&task_name) {
                    task.state = TaskState::Running;
                    task.start_time = Some(timestamp);
                }
            }
            TaskEvent::Progress { task_name, message } => {
                if let Some(task) = tasks.get_mut(&task_name) {
                    task.message = Some(message);

                    // Check if this is a cache hit message
                    if task
                        .message
                        .as_ref()
                        .is_some_and(|m| m.contains("cache hit"))
                    {
                        task.is_skipped = true;
                        task.skip_reason = Some("Already cached".to_string());
                        task.state = TaskState::Completed;
                        task.end_time = Some(Instant::now());

                        let mut completed = self.completed_tasks.write().await;
                        *completed += 1;
                    }
                }
            }
            TaskEvent::Completed { task_name, .. } => {
                if let Some(task) = tasks.get_mut(&task_name) {
                    if !task.is_skipped {
                        task.state = TaskState::Completed;
                        task.end_time = Some(Instant::now());

                        let mut completed = self.completed_tasks.write().await;
                        *completed += 1;
                    }
                }
            }
            TaskEvent::Failed {
                task_name, error, ..
            } => {
                if let Some(task) = tasks.get_mut(&task_name) {
                    task.state = TaskState::Failed;
                    task.message = Some(error);
                    task.end_time = Some(Instant::now());

                    let mut failed = self.failed_tasks.write().await;
                    *failed += 1;

                    let mut completed = self.completed_tasks.write().await;
                    *completed += 1;
                }
            }
            TaskEvent::Cancelled { task_name } => {
                if let Some(task) = tasks.get_mut(&task_name) {
                    task.state = TaskState::Cancelled;
                    task.end_time = Some(Instant::now());
                }
            }
            _ => {}
        }

        // Update spinner frames for running tasks
        for task in tasks.values_mut() {
            if task.state == TaskState::Running {
                task.spinner_frame += 1;
            }
        }

        drop(tasks);

        // Redraw the display
        self.draw_all().await?;

        Ok(())
    }

    /// Update spinner animation
    pub async fn tick(&self) -> io::Result<()> {
        let mut tasks = self.tasks.write().await;

        // Update spinner frames
        for task in tasks.values_mut() {
            if task.state == TaskState::Running {
                task.spinner_frame += 1;
            }
        }

        drop(tasks);

        // Redraw
        self.draw_all().await
    }

    /// Cleanup when done
    pub fn cleanup(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        stdout.execute(Show)?;
        stdout.execute(SetAttribute(Attribute::Reset))?;
        stdout.execute(ResetColor)?;
        writeln!(stdout)?;
        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{TaskEvent, TaskRegistry, TaskState};
    use cuenv_core::types::tasks::TaskDefinition;
    use cuenv_task::TaskExecutionPlan;
    use std::collections::HashMap;
    use std::time::Instant;

    fn create_test_task_definition(name: &str, dependencies: Vec<String>) -> TaskDefinition {
        use cuenv_core::types::tasks::{ResolvedDependency, TaskExecutionMode};
        use std::path::PathBuf;
        use std::time::Duration;

        let resolved_deps = dependencies
            .into_iter()
            .map(ResolvedDependency::new)
            .collect();

        TaskDefinition {
            name: name.to_string(),
            description: Some(format!("Test task {name}")),
            execution_mode: TaskExecutionMode::Command {
                command: format!("echo 'Running {name}'"),
            },
            dependencies: resolved_deps,
            working_directory: PathBuf::from("/tmp"),
            shell: "sh".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            security: None,
            cache: Default::default(),
            timeout: Duration::from_secs(60),
        }
    }

    fn create_test_execution_plan() -> TaskExecutionPlan {
        let mut tasks = HashMap::new();
        tasks.insert(
            "task1".to_string(),
            create_test_task_definition("task1", vec![]),
        );
        tasks.insert(
            "task2".to_string(),
            create_test_task_definition("task2", vec!["task1".to_string()]),
        );
        tasks.insert(
            "task3".to_string(),
            create_test_task_definition("task3", vec![]),
        );

        let levels = vec![
            vec!["task1".to_string(), "task3".to_string()],
            vec!["task2".to_string()],
        ];

        TaskExecutionPlan { tasks, levels }
    }

    #[test]
    fn test_task_display_new() {
        let dependencies = vec!["dep1".to_string(), "dep2".to_string()];
        let display = TaskDisplay::new("test_task".to_string(), 2, dependencies.clone());

        assert_eq!(display.name, "test_task");
        assert_eq!(display.state, TaskState::Queued);
        assert_eq!(display.depth, 2);
        assert_eq!(display.dependencies, dependencies);
        assert_eq!(display.message, None);
        assert_eq!(display.progress, None);
        assert_eq!(display.start_time, None);
        assert_eq!(display.end_time, None);
        assert_eq!(display.line_number, None);
        assert_eq!(display.spinner_frame, 0);
        assert!(!display.is_skipped);
        assert_eq!(display.skip_reason, None);
    }

    #[test]
    fn test_task_display_duration_str() {
        let mut display = TaskDisplay::new("test".to_string(), 0, vec![]);

        // No times set
        assert_eq!(display.duration_str(), "0.0s");

        let start_time = Instant::now();
        display.start_time = Some(start_time);

        // Only start time set
        let duration = display.duration_str();
        assert!(duration.ends_with('s'));
        let duration_val = duration.trim_end_matches('s');
        assert!(duration_val.parse::<f32>().is_ok());

        // Both start and end time set
        let end_time = start_time + std::time::Duration::from_millis(1500);
        display.end_time = Some(end_time);
        assert_eq!(display.duration_str(), "1.5s");
    }

    #[test]
    fn test_task_display_status_icon() {
        let mut display = TaskDisplay::new("test".to_string(), 0, vec![]);

        assert_eq!(display.status_icon(), "◌");

        display.state = TaskState::Running;
        display.spinner_frame = 0;
        assert_eq!(display.status_icon(), SPINNER_FRAMES[0]);

        display.spinner_frame = 5;
        assert_eq!(display.status_icon(), SPINNER_FRAMES[5]);

        display.state = TaskState::Completed;
        assert_eq!(display.status_icon(), "✔");

        display.state = TaskState::Failed;
        assert_eq!(display.status_icon(), "✖");

        display.state = TaskState::Cancelled;
        assert_eq!(display.status_icon(), "⊘");

        // Test skipped task overrides state
        display.is_skipped = true;
        display.state = TaskState::Failed;
        assert_eq!(display.status_icon(), "✔");
    }

    #[test]
    fn test_task_display_status_color() {
        let mut display = TaskDisplay::new("test".to_string(), 0, vec![]);

        assert_eq!(display.status_color(), Color::DarkGrey);

        display.state = TaskState::Running;
        assert_eq!(display.status_color(), Color::Blue);

        display.state = TaskState::Completed;
        assert_eq!(display.status_color(), Color::Green);

        display.state = TaskState::Failed;
        assert_eq!(display.status_color(), Color::Red);

        display.state = TaskState::Cancelled;
        assert_eq!(display.status_color(), Color::DarkRed);

        // Test skipped task overrides state color
        display.is_skipped = true;
        display.state = TaskState::Failed;
        assert_eq!(display.status_color(), Color::Yellow);
    }

    #[test]
    fn test_task_display_format_progress_bar() {
        let mut display = TaskDisplay::new("test".to_string(), 0, vec![]);

        // No progress set
        assert_eq!(display.format_progress_bar(10), "");

        // With progress percentage
        display.progress = Some(50.0);
        let bar = display.format_progress_bar(10);
        assert!(bar.starts_with('['));
        assert!(bar.ends_with(']'));
        assert!(bar.contains(PROGRESS_FULL));
        assert!(bar.contains(PROGRESS_EMPTY));

        // Test edge cases
        display.progress = Some(0.0);
        let bar = display.format_progress_bar(10);
        assert_eq!(bar, format!("[{}]", PROGRESS_EMPTY.repeat(10)));

        display.progress = Some(100.0);
        let bar = display.format_progress_bar(10);
        assert_eq!(bar, format!("[{}]", PROGRESS_FULL.repeat(10)));

        // Test running state without progress
        display.progress = None;
        display.state = TaskState::Running;
        display.spinner_frame = 0;
        let bar = display.format_progress_bar(10);
        assert!(bar.starts_with('['));
        assert!(bar.ends_with(']'));
        assert!(bar.contains(PROGRESS_FULL));
    }

    #[test]
    fn test_task_display_format_progress_bar_animation() {
        let mut display = TaskDisplay::new("test".to_string(), 0, vec![]);
        display.state = TaskState::Running;

        let width = 5;

        // Test wave effect moves with spinner frame
        display.spinner_frame = 0;
        let bar1 = display.format_progress_bar(width);

        display.spinner_frame = 1;
        let bar2 = display.format_progress_bar(width);

        // The bars should be different due to animation
        assert_ne!(bar1, bar2);
        assert!(bar1.len() > 2); // Should have brackets
        assert!(bar2.len() > 2);
    }

    #[test]
    fn test_spinner_frames_constant() {
        assert_eq!(SPINNER_FRAMES.len(), 10);
        assert!(SPINNER_FRAMES.iter().all(|&frame| !frame.is_empty()));

        // Check some known spinner frames
        assert_eq!(SPINNER_FRAMES[0], "⠋");
        assert_eq!(SPINNER_FRAMES[1], "⠙");
        assert_eq!(SPINNER_FRAMES[9], "⠏");
    }

    #[test]
    fn test_progress_constants() {
        assert_eq!(PROGRESS_FULL, "█");
        assert_eq!(PROGRESS_EMPTY, "░");
    }

    #[tokio::test]
    async fn test_spinner_formatter_new() {
        let registry = TaskRegistry::new();
        let formatter = SpinnerFormatter::new(registry);

        assert_eq!(formatter.task_order.len(), 0);
        assert_eq!(formatter.start_line, 0);
        assert_eq!(formatter.total_tasks, 0);
        assert_eq!(*formatter.completed_tasks.read().await, 0);
        assert_eq!(*formatter.failed_tasks.read().await, 0);
    }

    #[tokio::test]
    async fn test_spinner_formatter_initialize() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);
        let plan = create_test_execution_plan();

        // Note: This will attempt to write to stdout, which might fail in test environment
        // but we're testing the initialization logic, not the actual terminal output
        let _result = formatter.initialize(&plan).await;

        // The result might fail due to terminal unavailability in tests, but we can check state
        assert_eq!(formatter.total_tasks, 3);
        assert_eq!(formatter.task_order.len(), 3);

        // Check that tasks were initialized
        let tasks = formatter.tasks.read().await;
        assert_eq!(tasks.len(), 3);
        assert!(tasks.contains_key("task1"));
        assert!(tasks.contains_key("task2"));
        assert!(tasks.contains_key("task3"));

        // Check task ordering respects dependencies
        let task2_index = formatter
            .task_order
            .iter()
            .position(|t| t == "task2")
            .unwrap();
        let task1_index = formatter
            .task_order
            .iter()
            .position(|t| t == "task1")
            .unwrap();
        // task2 depends on task1, so it should come after
        assert!(task2_index > task1_index);
    }

    #[tokio::test]
    async fn test_spinner_formatter_build_display_order() {
        let plan = create_test_execution_plan();
        let mut order = Vec::new();

        SpinnerFormatter::build_display_order(&plan, &mut order);

        assert_eq!(order.len(), 3);
        assert!(order.contains(&"task1".to_string()));
        assert!(order.contains(&"task2".to_string()));
        assert!(order.contains(&"task3".to_string()));

        // Check dependency ordering
        let task1_pos = order.iter().position(|t| t == "task1").unwrap();
        let task2_pos = order.iter().position(|t| t == "task2").unwrap();
        assert!(
            task1_pos < task2_pos,
            "task1 should come before task2 due to dependency"
        );
    }

    #[tokio::test]
    async fn test_spinner_formatter_handle_task_started_event() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);
        let plan = create_test_execution_plan();

        // Initialize with empty setup to avoid terminal operations
        formatter.total_tasks = plan.tasks.len();
        formatter.task_order = vec!["task1".to_string()];

        let mut tasks = HashMap::new();
        let display = TaskDisplay::new("task1".to_string(), 0, vec![]);
        tasks.insert("task1".to_string(), display);
        *formatter.tasks.write().await = tasks;

        let timestamp = Instant::now();
        let event = TaskEvent::Started {
            task_name: "task1".to_string(),
            timestamp,
        };

        // This will fail due to terminal output, but we can check the state change
        let _ = formatter.handle_event(event).await;

        let tasks = formatter.tasks.read().await;
        let task = tasks.get("task1").unwrap();
        assert_eq!(task.state, TaskState::Running);
        assert_eq!(task.start_time, Some(timestamp));
    }

    #[tokio::test]
    async fn test_spinner_formatter_handle_task_completed_event() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);
        let plan = create_test_execution_plan();

        // Initialize with empty setup
        formatter.total_tasks = plan.tasks.len();
        formatter.task_order = vec!["task1".to_string()];

        let mut tasks = HashMap::new();
        let mut display = TaskDisplay::new("task1".to_string(), 0, vec![]);
        display.state = TaskState::Running;
        display.start_time = Some(Instant::now());
        tasks.insert("task1".to_string(), display);
        *formatter.tasks.write().await = tasks;

        let event = TaskEvent::Completed {
            task_name: "task1".to_string(),
            exit_code: 0,
            duration_ms: 1000,
        };

        let _ = formatter.handle_event(event).await;

        let tasks = formatter.tasks.read().await;
        let task = tasks.get("task1").unwrap();
        assert_eq!(task.state, TaskState::Completed);
        assert!(task.end_time.is_some());

        let completed_count = *formatter.completed_tasks.read().await;
        assert_eq!(completed_count, 1);
    }

    #[tokio::test]
    async fn test_spinner_formatter_handle_task_failed_event() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);

        formatter.total_tasks = 1;
        formatter.task_order = vec!["task1".to_string()];

        let mut tasks = HashMap::new();
        let mut display = TaskDisplay::new("task1".to_string(), 0, vec![]);
        display.state = TaskState::Running;
        tasks.insert("task1".to_string(), display);
        *formatter.tasks.write().await = tasks;

        let event = TaskEvent::Failed {
            task_name: "task1".to_string(),
            error: "Test error".to_string(),
            duration_ms: 500,
        };

        let _ = formatter.handle_event(event).await;

        let tasks = formatter.tasks.read().await;
        let task = tasks.get("task1").unwrap();
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.message, Some("Test error".to_string()));
        assert!(task.end_time.is_some());

        let failed_count = *formatter.failed_tasks.read().await;
        assert_eq!(failed_count, 1);

        let completed_count = *formatter.completed_tasks.read().await;
        assert_eq!(completed_count, 1);
    }

    #[tokio::test]
    async fn test_spinner_formatter_handle_task_cancelled_event() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);

        formatter.task_order = vec!["task1".to_string()];

        let mut tasks = HashMap::new();
        let display = TaskDisplay::new("task1".to_string(), 0, vec![]);
        tasks.insert("task1".to_string(), display);
        *formatter.tasks.write().await = tasks;

        let event = TaskEvent::Cancelled {
            task_name: "task1".to_string(),
        };

        let _ = formatter.handle_event(event).await;

        let tasks = formatter.tasks.read().await;
        let task = tasks.get("task1").unwrap();
        assert_eq!(task.state, TaskState::Cancelled);
        assert!(task.end_time.is_some());
    }

    #[tokio::test]
    async fn test_spinner_formatter_handle_progress_event() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);

        formatter.task_order = vec!["task1".to_string()];

        let mut tasks = HashMap::new();
        let display = TaskDisplay::new("task1".to_string(), 0, vec![]);
        tasks.insert("task1".to_string(), display);
        *formatter.tasks.write().await = tasks;

        let event = TaskEvent::Progress {
            task_name: "task1".to_string(),
            message: "Processing...".to_string(),
        };

        let _ = formatter.handle_event(event).await;

        let tasks = formatter.tasks.read().await;
        let task = tasks.get("task1").unwrap();
        assert_eq!(task.message, Some("Processing...".to_string()));
    }

    #[tokio::test]
    async fn test_spinner_formatter_handle_cache_hit_progress() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);

        formatter.task_order = vec!["task1".to_string()];

        let mut tasks = HashMap::new();
        let display = TaskDisplay::new("task1".to_string(), 0, vec![]);
        tasks.insert("task1".to_string(), display);
        *formatter.tasks.write().await = tasks;

        let event = TaskEvent::Progress {
            task_name: "task1".to_string(),
            message: "cache hit - skipping execution".to_string(),
        };

        let _ = formatter.handle_event(event).await;

        let tasks = formatter.tasks.read().await;
        let task = tasks.get("task1").unwrap();
        assert!(task.is_skipped);
        assert_eq!(task.skip_reason, Some("Already cached".to_string()));
        assert_eq!(task.state, TaskState::Completed);
        assert!(task.end_time.is_some());

        let completed_count = *formatter.completed_tasks.read().await;
        assert_eq!(completed_count, 1);
    }

    #[tokio::test]
    async fn test_spinner_formatter_tick_updates_frames() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);

        formatter.task_order = vec!["task1".to_string()];

        let mut tasks = HashMap::new();
        let mut display = TaskDisplay::new("task1".to_string(), 0, vec![]);
        display.state = TaskState::Running;
        display.spinner_frame = 0;
        tasks.insert("task1".to_string(), display);
        *formatter.tasks.write().await = tasks;

        let _ = formatter.tick().await;

        let tasks = formatter.tasks.read().await;
        let task = tasks.get("task1").unwrap();
        assert_eq!(task.spinner_frame, 1);
    }

    #[tokio::test]
    async fn test_spinner_formatter_tick_only_updates_running_tasks() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);

        formatter.task_order = vec!["task1".to_string(), "task2".to_string()];

        let mut tasks = HashMap::new();

        let mut running_task = TaskDisplay::new("task1".to_string(), 0, vec![]);
        running_task.state = TaskState::Running;
        running_task.spinner_frame = 0;

        let mut completed_task = TaskDisplay::new("task2".to_string(), 0, vec![]);
        completed_task.state = TaskState::Completed;
        completed_task.spinner_frame = 0;

        tasks.insert("task1".to_string(), running_task);
        tasks.insert("task2".to_string(), completed_task);
        *formatter.tasks.write().await = tasks;

        let _ = formatter.tick().await;

        let tasks = formatter.tasks.read().await;
        assert_eq!(tasks.get("task1").unwrap().spinner_frame, 1);
        assert_eq!(tasks.get("task2").unwrap().spinner_frame, 0);
    }

    #[test]
    fn test_spinner_formatter_cleanup() {
        let registry = TaskRegistry::new();
        let formatter = SpinnerFormatter::new(registry);

        // This will likely fail in test environment due to no terminal,
        // but we're testing that the method exists and handles errors gracefully
        let _ = formatter.cleanup();
    }

    #[tokio::test]
    async fn test_concurrent_task_handling() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);

        formatter.task_order = vec![
            "task1".to_string(),
            "task2".to_string(),
            "task3".to_string(),
        ];

        let mut tasks = HashMap::new();
        for i in 1..=3 {
            let task_name = format!("task{i}");
            let display = TaskDisplay::new(task_name.clone(), 0, vec![]);
            tasks.insert(task_name, display);
        }
        *formatter.tasks.write().await = tasks;

        // Simulate concurrent events
        let events = vec![
            TaskEvent::Started {
                task_name: "task1".to_string(),
                timestamp: Instant::now(),
            },
            TaskEvent::Started {
                task_name: "task2".to_string(),
                timestamp: Instant::now(),
            },
            TaskEvent::Progress {
                task_name: "task1".to_string(),
                message: "Working...".to_string(),
            },
            TaskEvent::Completed {
                task_name: "task1".to_string(),
                exit_code: 0,
                duration_ms: 2000,
            },
        ];

        for event in events {
            let _ = formatter.handle_event(event).await;
        }

        let tasks = formatter.tasks.read().await;
        assert_eq!(tasks.get("task1").unwrap().state, TaskState::Completed);
        assert_eq!(tasks.get("task2").unwrap().state, TaskState::Running);
        assert_eq!(tasks.get("task3").unwrap().state, TaskState::Queued);

        let completed_count = *formatter.completed_tasks.read().await;
        assert_eq!(completed_count, 1);
    }

    #[tokio::test]
    async fn test_deep_task_hierarchy() {
        let registry = TaskRegistry::new();
        let mut formatter = SpinnerFormatter::new(registry);

        // Create tasks with different depths
        formatter.task_order = vec![
            "root".to_string(),
            "child".to_string(),
            "grandchild".to_string(),
        ];

        let mut tasks = HashMap::new();
        let root_display = TaskDisplay::new("root".to_string(), 0, vec![]);
        let child_display = TaskDisplay::new("child".to_string(), 1, vec!["root".to_string()]);
        let grandchild_display =
            TaskDisplay::new("grandchild".to_string(), 2, vec!["child".to_string()]);

        tasks.insert("root".to_string(), root_display);
        tasks.insert("child".to_string(), child_display);
        tasks.insert("grandchild".to_string(), grandchild_display);
        *formatter.tasks.write().await = tasks;

        // Test that all tasks can be accessed and have correct depths
        let tasks = formatter.tasks.read().await;
        assert_eq!(tasks.get("root").unwrap().depth, 0);
        assert_eq!(tasks.get("child").unwrap().depth, 1);
        assert_eq!(tasks.get("grandchild").unwrap().depth, 2);
    }

    #[test]
    fn test_edge_case_unicode_task_names() {
        let display = TaskDisplay::new("测试任务🚀".to_string(), 0, vec![]);
        assert_eq!(display.name, "测试任务🚀");
        assert_eq!(display.status_icon(), "◌");
    }

    #[test]
    fn test_edge_case_very_long_task_names() {
        let long_name = "a".repeat(1000);
        let display = TaskDisplay::new(long_name.clone(), 0, vec![]);
        assert_eq!(display.name, long_name);
    }

    #[test]
    fn test_edge_case_empty_task_name() {
        let display = TaskDisplay::new(String::new(), 0, vec![]);
        assert_eq!(display.name, "");
    }

    #[test]
    fn test_spinner_frame_overflow() {
        let mut display = TaskDisplay::new("test".to_string(), 0, vec![]);
        display.state = TaskState::Running;
        display.spinner_frame = SPINNER_FRAMES.len() * 2 + 5;

        let icon = display.status_icon();
        assert!(SPINNER_FRAMES.contains(&icon));
    }
}
