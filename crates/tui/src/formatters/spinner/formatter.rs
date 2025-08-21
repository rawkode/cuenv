//! SpinnerFormatter implementation for Docker Compose-style task display

use super::task_display::TaskDisplay;
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
