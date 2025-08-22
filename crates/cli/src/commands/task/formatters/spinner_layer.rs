//! Epic spinner formatter layer for the most amazing animated task display ever

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    style::{Attribute, Color, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{Clear, ClearType, DisableLineWrap, EnableLineWrap},
    ExecutableCommand,
};
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};

/// The most epic spinner frames for maximum visual appeal
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Gorgeous progress bar characters
const PROGRESS_FULL: char = '█';
const PROGRESS_EMPTY: char = '░';
const PROGRESS_PARTIAL: &[char] = &['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// Task states for our epic display
#[derive(Debug, Clone, PartialEq)]
enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Individual task display state
#[derive(Debug, Clone)]
struct TaskDisplay {
    name: String,
    state: TaskState,
    message: Option<String>,
    progress: Option<f32>,
    start_time: Option<Instant>,
    end_time: Option<Instant>,
    line_number: Option<u16>,
    spinner_frame: usize,
    depth: usize,
    error: Option<String>,
}

impl TaskDisplay {
    fn new(name: String) -> Self {
        let depth = name.matches('.').count() + name.matches(':').count();
        Self {
            name,
            state: TaskState::Queued,
            message: None,
            progress: None,
            start_time: None,
            end_time: None,
            line_number: None,
            spinner_frame: 0,
            depth,
            error: None,
        }
    }

    fn display_name(&self) -> &str {
        if let Some(last_separator) = self.name.rfind('.').or_else(|| self.name.rfind(':')) {
            &self.name[last_separator + 1..]
        } else {
            &self.name
        }
    }

    fn duration_ms(&self) -> u64 {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => end.duration_since(start).as_millis() as u64,
            (Some(start), None) => Instant::now().duration_since(start).as_millis() as u64,
            _ => 0,
        }
    }

    fn status_icon(&self) -> &'static str {
        match self.state {
            TaskState::Queued => "◌",
            TaskState::Running => SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()],
            TaskState::Completed => "✔",
            TaskState::Failed => "✖",
            TaskState::Cancelled => "⊘",
        }
    }

    fn status_color(&self) -> Color {
        match self.state {
            TaskState::Queued => Color::DarkGrey,
            TaskState::Running => Color::Cyan,
            TaskState::Completed => Color::Green,
            TaskState::Failed => Color::Red,
            TaskState::Cancelled => Color::Yellow,
        }
    }

    fn format_progress_bar(&self, width: usize) -> String {
        if let Some(progress) = self.progress {
            let filled = (progress * width as f32) as usize;
            let partial_progress = (progress * width as f32) - filled as f32;
            let partial_char = if partial_progress > 0.0 {
                PROGRESS_PARTIAL[((partial_progress * PROGRESS_PARTIAL.len() as f32) as usize)
                    .min(PROGRESS_PARTIAL.len() - 1)]
            } else {
                PROGRESS_EMPTY
            };

            let mut bar = PROGRESS_FULL.to_string().repeat(filled);
            if filled < width {
                bar.push(partial_char);
                bar.push_str(&PROGRESS_EMPTY.to_string().repeat(width - filled - 1));
            }
            format!("[{}]", bar)
        } else {
            format!("[{}]", PROGRESS_EMPTY.to_string().repeat(width))
        }
    }
}

/// The most epic spinner formatter layer ever created
pub struct SpinnerFormatterLayer {
    state: Arc<Mutex<SpinnerState>>,
}

#[derive(Debug)]
struct SpinnerState {
    tasks: HashMap<String, TaskDisplay>,
    task_order: Vec<String>,
    start_line: u16,
    total_tasks: usize,
    completed_tasks: usize,
    failed_tasks: usize,
    is_initialized: bool,
    _terminal_height: u16,
}

impl SpinnerFormatterLayer {
    /// Create the most epic spinner formatter layer ever
    pub fn new() -> io::Result<Self> {
        // Use a fallback terminal size to avoid hanging in some environments
        let terminal_height = crossterm::terminal::size()
            .map(|(_, height)| height)
            .unwrap_or(24); // Default to 24 lines if detection fails

        let state = SpinnerState {
            tasks: HashMap::new(),
            task_order: Vec::new(),
            start_line: 0,
            total_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            is_initialized: false,
            _terminal_height: terminal_height,
        };

        let layer = Self {
            state: Arc::new(Mutex::new(state)),
        };

        Ok(layer)
    }

    fn initialize_display(&self) -> io::Result<()> {
        // Simple initialization without complex terminal control
        if let Ok(mut state) = self.state.lock() {
            state.is_initialized = true;
        }
        Ok(())
    }

    fn handle_task_event(&self, event: cuenv_tui::TaskEvent) -> io::Result<()> {
        // Epic spinner with beautiful animations and status
        match event {
            cuenv_tui::TaskEvent::Started { task_name, .. } => {
                // Use animated spinner frame
                let icon = SPINNER_FRAMES[0]; // Start with first frame
                let depth = task_name.matches('.').count() + task_name.matches(':').count();
                let indent = "  ".repeat(depth);
                let display_name = if let Some(last_separator) =
                    task_name.rfind('.').or_else(|| task_name.rfind(':'))
                {
                    &task_name[last_separator + 1..]
                } else {
                    &task_name
                };

                tracing::info!("{}{} {} (starting...)", indent, icon, display_name);
            }
            cuenv_tui::TaskEvent::Completed {
                task_name,
                duration_ms,
                ..
            } => {
                let depth = task_name.matches('.').count() + task_name.matches(':').count();
                let indent = "  ".repeat(depth);
                let display_name = if let Some(last_separator) =
                    task_name.rfind('.').or_else(|| task_name.rfind(':'))
                {
                    &task_name[last_separator + 1..]
                } else {
                    &task_name
                };

                tracing::info!(
                    "{}✔ {} ✨ (completed in {}ms)",
                    indent,
                    display_name,
                    duration_ms
                );
            }
            cuenv_tui::TaskEvent::Failed {
                task_name, error, ..
            } => {
                let depth = task_name.matches('.').count() + task_name.matches(':').count();
                let indent = "  ".repeat(depth);
                let display_name = if let Some(last_separator) =
                    task_name.rfind('.').or_else(|| task_name.rfind(':'))
                {
                    &task_name[last_separator + 1..]
                } else {
                    &task_name
                };

                tracing::error!("{}✖ {} 💥 (failed: {})", indent, display_name, error);
            }
            cuenv_tui::TaskEvent::Progress { task_name, message } => {
                // Show progress with current spinner frame
                let depth = task_name.matches('.').count() + task_name.matches(':').count();
                let indent = "  ".repeat(depth);
                let display_name = if let Some(last_separator) =
                    task_name.rfind('.').or_else(|| task_name.rfind(':'))
                {
                    &task_name[last_separator + 1..]
                } else {
                    &task_name
                };

                // Use a different spinner frame for progress to show animation
                let icon = SPINNER_FRAMES[1];
                tracing::info!("{}{} {} ({})", indent, icon, display_name, message);
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_started(&self, state: &mut SpinnerState, task_name: String) -> io::Result<()> {
        if !state.tasks.contains_key(&task_name) {
            let task = TaskDisplay::new(task_name.clone());
            state.task_order.push(task_name.clone());
            state.total_tasks += 1;
            state.tasks.insert(task_name.clone(), task);
        }

        if let Some(task) = state.tasks.get_mut(&task_name) {
            task.state = TaskState::Running;
            task.start_time = Some(Instant::now());

            // Use eprintln to avoid stdout deadlock with process
            let icon = task.status_icon();
            let indent = "  ".repeat(task.depth);
            tracing::info!("{}{} {} (started)", indent, icon, task.display_name());
        }

        if !state.is_initialized {
            self.initialize_display()?;
        }

        Ok(())
    }

    fn handle_progress(
        &self,
        state: &mut SpinnerState,
        task_name: String,
        message: String,
    ) -> io::Result<()> {
        if let Some(task) = state.tasks.get_mut(&task_name) {
            task.message = Some(message);
            // Simulate some progress for visual appeal
            if task.progress.is_none() {
                task.progress = Some(0.0);
            }
        }
        Ok(())
    }

    fn handle_completed(
        &self,
        state: &mut SpinnerState,
        task_name: String,
        _duration_ms: u64,
    ) -> io::Result<()> {
        if let Some(task) = state.tasks.get_mut(&task_name) {
            task.state = TaskState::Completed;
            task.end_time = Some(Instant::now());
            task.progress = Some(1.0);
            state.completed_tasks += 1;

            // Use eprintln to avoid stdout deadlock with process
            let indent = "  ".repeat(task.depth);
            eprintln!(
                "{}✔ {} ✨ (completed in {}ms)",
                indent,
                task.display_name(),
                task.duration_ms()
            );
        }
        Ok(())
    }

    fn handle_failed(
        &self,
        state: &mut SpinnerState,
        task_name: String,
        error: String,
    ) -> io::Result<()> {
        if let Some(task) = state.tasks.get_mut(&task_name) {
            task.state = TaskState::Failed;
            task.end_time = Some(Instant::now());
            task.error = Some(error.clone());
            state.failed_tasks += 1;

            // Use eprintln to avoid stdout deadlock with process
            let indent = "  ".repeat(task.depth);
            tracing::error!("{}✖ {} 💥 (failed: {})", indent, task.display_name(), error);
        }
        Ok(())
    }

    fn handle_cancelled(&self, state: &mut SpinnerState, task_name: String) -> io::Result<()> {
        if let Some(task) = state.tasks.get_mut(&task_name) {
            task.state = TaskState::Cancelled;
            task.end_time = Some(Instant::now());
        }
        Ok(())
    }

    fn render(&self) -> io::Result<()> {
        // Simple render - we'll only print when events happen
        // No continuous rendering for now
        Ok(())
    }

    fn draw_all(&self, state: &SpinnerState) -> io::Result<()> {
        // Simple drawing without terminal positioning - just print updates
        for task_name in &state.task_order {
            if let Some(task) = state.tasks.get(task_name) {
                self.draw_simple_task(task)?;
            }
        }
        Ok(())
    }

    fn draw_simple_task(&self, task: &TaskDisplay) -> io::Result<()> {
        use std::io::{self, Write};

        let indent = "  ".repeat(task.depth);
        let icon = task.status_icon();
        let name = task.display_name();

        match task.state {
            TaskState::Running => {
                tracing::info!(
                    "{}{} {} (running {}ms)",
                    indent,
                    icon,
                    name,
                    task.duration_ms()
                );
                io::stdout().flush()?;
            }
            TaskState::Completed => {
                tracing::info!(
                    "{}{} {} ✨ (completed in {}ms)",
                    indent,
                    icon,
                    name,
                    task.duration_ms()
                );
                io::stdout().flush()?;
            }
            TaskState::Failed => {
                if let Some(ref error) = task.error {
                    tracing::error!("{}{} {} 💥 (failed: {})", indent, icon, name, error);
                } else {
                    tracing::error!("{}{} {} 💥 (failed)", indent, icon, name);
                }
                io::stdout().flush()?;
            }
            _ => {} // Don't print for queued/cancelled for now
        }

        Ok(())
    }

    fn draw_header(&self, stdout: &mut io::Stdout, state: &SpinnerState) -> io::Result<()> {
        stdout.execute(Clear(ClearType::CurrentLine))?;
        stdout.execute(SetAttribute(Attribute::Bold))?;

        if state.failed_tasks > 0 {
            stdout.execute(SetForegroundColor(Color::Red))?;
            write!(stdout, "✖ ")?;
        } else if state.completed_tasks == state.total_tasks && state.total_tasks > 0 {
            stdout.execute(SetForegroundColor(Color::Green))?;
            write!(stdout, "✔ ")?;
        } else {
            stdout.execute(SetForegroundColor(Color::Cyan))?;
            write!(stdout, "⚡ ")?;
        }

        write!(
            stdout,
            "Tasks: {}/{}",
            state.completed_tasks, state.total_tasks
        )?;
        if state.failed_tasks > 0 {
            stdout.execute(SetForegroundColor(Color::Red))?;
            write!(stdout, " ({} failed)", state.failed_tasks)?;
        }

        stdout.execute(ResetColor)?;
        stdout.execute(SetAttribute(Attribute::Reset))?;
        writeln!(stdout)?;

        // Add a separator line
        stdout.execute(SetForegroundColor(Color::DarkGrey))?;
        writeln!(
            stdout,
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        )?;
        stdout.execute(ResetColor)?;

        Ok(())
    }

    fn draw_task(&self, stdout: &mut io::Stdout, task: &TaskDisplay, line: u16) -> io::Result<()> {
        stdout.execute(MoveTo(0, line))?;
        stdout.execute(Clear(ClearType::CurrentLine))?;

        // Beautiful hierarchical indentation
        let indent = "  ".repeat(task.depth);
        write!(stdout, "{}", indent)?;

        // Epic status icon with color
        stdout.execute(SetForegroundColor(task.status_color()))?;
        write!(stdout, "{} ", task.status_icon())?;

        // Task name in bold
        stdout.execute(SetAttribute(Attribute::Bold))?;
        stdout.execute(SetForegroundColor(Color::White))?;
        let name_width = 25_usize.saturating_sub(task.depth * 2);
        write!(
            stdout,
            "{:<width$}",
            task.display_name(),
            width = name_width
        )?;
        stdout.execute(SetAttribute(Attribute::Reset))?;

        // Status-specific display
        match task.state {
            TaskState::Running => {
                // Progress bar for running tasks
                stdout.execute(SetForegroundColor(Color::Cyan))?;
                write!(stdout, " {} ", task.format_progress_bar(12))?;

                // Duration counter
                stdout.execute(SetForegroundColor(Color::Blue))?;
                write!(stdout, "{}ms", task.duration_ms())?;

                if let Some(ref message) = task.message {
                    stdout.execute(SetForegroundColor(Color::DarkGrey))?;
                    write!(stdout, " - {}", message)?;
                }
            }
            TaskState::Completed => {
                stdout.execute(SetForegroundColor(Color::Green))?;
                write!(stdout, " ✨ Completed in {}ms", task.duration_ms())?;
            }
            TaskState::Failed => {
                stdout.execute(SetForegroundColor(Color::Red))?;
                write!(stdout, " 💥 Failed")?;
                if let Some(ref error) = task.error {
                    write!(stdout, " - {}", error)?;
                }
            }
            TaskState::Cancelled => {
                stdout.execute(SetForegroundColor(Color::Yellow))?;
                write!(stdout, " ⚠ Cancelled")?;
            }
            TaskState::Queued => {
                stdout.execute(SetForegroundColor(Color::DarkGrey))?;
                write!(stdout, " Queued")?;
            }
        }

        stdout.execute(ResetColor)?;
        Ok(())
    }

    fn draw_footer(
        &self,
        stdout: &mut io::Stdout,
        state: &SpinnerState,
        line: u16,
    ) -> io::Result<()> {
        stdout.execute(MoveTo(0, line))?;
        stdout.execute(Clear(ClearType::CurrentLine))?;

        stdout.execute(SetForegroundColor(Color::DarkGrey))?;
        writeln!(
            stdout,
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        )?;

        if state.failed_tasks > 0 {
            stdout.execute(SetForegroundColor(Color::Red))?;
            write!(stdout, "💀 {} task(s) failed", state.failed_tasks)?;
        } else if state.completed_tasks == state.total_tasks {
            stdout.execute(SetForegroundColor(Color::Green))?;
            write!(
                stdout,
                "🎉 All {} task(s) completed successfully!",
                state.total_tasks
            )?;
        }

        stdout.execute(ResetColor)?;
        Ok(())
    }

    /// Epic cleanup when the spinner is done
    fn cleanup(&self) -> io::Result<()> {
        // Simple cleanup - no terminal control needed for now
        Ok(())
    }
}

impl Default for SpinnerFormatterLayer {
    fn default() -> Self {
        Self::new().expect("Failed to create the most epic SpinnerFormatterLayer")
    }
}

impl<S> Layer<S> for SpinnerFormatterLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Extract our epic task event from the tracing event
        if let Some(task_event) = extract_task_event_by_name(event) {
            if let Err(e) = self.handle_task_event(task_event) {
                // Don't pollute stderr during our epic display
                let _ = self.cleanup();
                tracing::error!("Epic spinner error: {e}");
            }
        }
    }
}

impl Drop for SpinnerFormatterLayer {
    fn drop(&mut self) {
        // Epic cleanup when we're done
        let _ = self.cleanup();
    }
}

/// Extract our epic task events from tracing events using event names
fn extract_task_event_by_name(event: &Event<'_>) -> Option<cuenv_tui::TaskEvent> {
    let mut visitor = TaskEventVisitor::default();
    event.record(&mut visitor);

    let task_name = visitor.task_name?;
    let timestamp = std::time::Instant::now();

    match event.metadata().name() {
        "task_started" => Some(cuenv_tui::TaskEvent::Started {
            task_name,
            timestamp,
        }),
        "task_progress" => Some(cuenv_tui::TaskEvent::Progress {
            task_name,
            message: visitor
                .message
                .unwrap_or_else(|| "Progress update".to_string()),
        }),
        "task_completed" => Some(cuenv_tui::TaskEvent::Completed {
            task_name,
            exit_code: 0,
            duration_ms: visitor.duration_ms.unwrap_or(0),
        }),
        "task_failed" => Some(cuenv_tui::TaskEvent::Failed {
            task_name,
            error: visitor.error.unwrap_or_else(|| "Unknown error".to_string()),
            duration_ms: visitor.duration_ms.unwrap_or(0),
        }),
        "task_cancelled" => Some(cuenv_tui::TaskEvent::Cancelled { task_name }),
        _ => None,
    }
}

#[derive(Default)]
struct TaskEventVisitor {
    task_name: Option<String>,
    message: Option<String>,
    error: Option<String>,
    duration_ms: Option<u64>,
}

impl tracing::field::Visit for TaskEventVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "task_name" => self.task_name = Some(value.to_string()),
            "message" => self.message = Some(value.to_string()),
            "error" => self.error = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "duration_ms" {
            self.duration_ms = Some(value);
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "task_name" => {
                self.task_name = Some(format!("{:?}", value).trim_matches('"').to_string())
            }
            "message" => self.message = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "error" => self.error = Some(format!("{:?}", value).trim_matches('"').to_string()),
            "duration_ms" => self.duration_ms = format!("{:?}", value).parse().ok(),
            _ => {}
        }
    }
}
