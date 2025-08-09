use super::{
    progress::ProgressTracker,
    task_span::{TaskSpan, TaskState},
    tree_formatter::TreeFormatter,
};
use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};
use dashmap::DashMap;
use std::{
    io::{stderr, Write},
    sync::Arc,
};
use tracing::{Event, Id, Subscriber};
use tracing_subscriber::{layer::Context, Layer};

/// A tracing subscriber layer that displays task execution as a real-time tree view
pub struct TreeSubscriber {
    /// Task states indexed by span ID
    tasks: Arc<DashMap<u64, TaskSpan>>,
    /// Progress tracker for throttling updates
    progress_tracker: ProgressTracker,
    /// Tree formatter for rendering
    formatter: TreeFormatter,
    /// Last rendered output for differential updates
    last_output: Arc<parking_lot::Mutex<String>>,
    /// Number of lines in last output (for clearing)
    last_line_count: Arc<parking_lot::Mutex<usize>>,
}

impl TreeSubscriber {
    /// Create a new tree subscriber
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
            progress_tracker: ProgressTracker::with_throttle(100), // 100ms throttle
            formatter: TreeFormatter::new(),
            last_output: Arc::new(parking_lot::Mutex::new(String::new())),
            last_line_count: Arc::new(parking_lot::Mutex::new(0)),
        }
    }

    /// Update the terminal display
    fn update_display(&self) {
        if !self.progress_tracker.should_update() {
            return;
        }

        let tasks = self.tasks.clone();
        let task_map = tasks
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect();

        // Generate the new tree output
        let tree_output = self.formatter.format_tree(&task_map);
        let summary = self.formatter.format_summary(&task_map);
        let full_output = format!("{}\n{}", summary, tree_output);

        // Check if output has changed
        let mut last_output = self.last_output.lock();
        if *last_output == full_output {
            return;
        }

        // Clear previous output
        let last_line_count = *self.last_line_count.lock();
        if last_line_count > 0 {
            let _ = execute!(
                stderr(),
                cursor::MoveUp(last_line_count as u16),
                terminal::Clear(ClearType::FromCursorDown)
            );
        }

        // Write new output
        let lines: Vec<&str> = full_output.lines().collect();
        let terminal_width = self.formatter.get_terminal_width();

        for line in &lines {
            let truncated = self.formatter.truncate_line(line, terminal_width);
            let _ = writeln!(stderr(), "{}", truncated);
        }

        // Update tracking variables
        *last_output = full_output.clone();
        *self.last_line_count.lock() = lines.len();
    }

    /// Force a final display update
    pub fn final_update(&self) {
        self.progress_tracker.force_update();
        self.update_display();

        // Add a blank line after the final output
        let _ = writeln!(stderr());
    }

    /// Extract task name from span attributes
    fn extract_task_name(&self, attrs: &tracing::span::Attributes<'_>) -> Option<String> {
        // This is a simplified version. In practice, we'd need to properly
        // extract field values from the span attributes.
        // For now, we'll use the span name as a fallback.
        Some(attrs.metadata().name().to_string())
    }

    /// Extract parent span ID from the current context
    fn extract_parent_id<S>(&self, ctx: &Context<'_, S>) -> Option<u64>
    where
        S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    {
        ctx.lookup_current()
            .and_then(|current| current.parent())
            .map(|parent| parent.id().into_u64())
    }
}

impl Default for TreeSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for TreeSubscriber
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let metadata = attrs.metadata();

        // Only track task, level, and pipeline spans
        if !matches!(metadata.name(), "task" | "level" | "pipeline") {
            return;
        }

        let span_id = id.into_u64();
        let parent_id = self.extract_parent_id(&ctx);

        if metadata.name() == "task" {
            let task_name = self
                .extract_task_name(attrs)
                .unwrap_or_else(|| format!("task-{}", span_id));

            let task_span = TaskSpan::new(task_name, parent_id);

            // Add this task as a child to its parent
            if let Some(parent_span_id) = parent_id {
                if let Some(mut parent) = self.tasks.get_mut(&parent_span_id) {
                    parent.add_child(span_id);
                }
            }

            self.tasks.insert(span_id, task_span);
            self.update_display();
        }
    }

    fn on_enter(&self, id: &Id, _ctx: Context<'_, S>) {
        let span_id = id.into_u64();

        if let Some(mut task) = self.tasks.get_mut(&span_id) {
            if matches!(task.state, TaskState::Waiting) {
                task.start();
                self.update_display();
            }
        }
    }

    fn on_exit(&self, _id: &Id, _ctx: Context<'_, S>) {
        // Tasks don't automatically complete on span exit
        // They need explicit completion events
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        let span_id = id.into_u64();

        if let Some(mut task) = self.tasks.get_mut(&span_id) {
            // If the task is still running when the span closes, mark it as completed
            if matches!(task.state, TaskState::Running { .. }) {
                task.complete();
                self.update_display();
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();

        // Handle different types of structured events
        match metadata.name() {
            "task_progress" => {
                if let Some(current) = ctx.lookup_current() {
                    let span_id = current.id().into_u64();
                    if let Some(_task) = self.tasks.get_mut(&span_id) {
                        // In practice, we'd extract progress and message from event fields
                        // This is a simplified version
                        self.update_display();
                    }
                }
            }
            "task_completed" => {
                if let Some(current) = ctx.lookup_current() {
                    let span_id = current.id().into_u64();
                    if let Some(mut task) = self.tasks.get_mut(&span_id) {
                        task.complete();
                        self.update_display();
                    }
                }
            }
            "task_failed" => {
                if let Some(current) = ctx.lookup_current() {
                    let span_id = current.id().into_u64();
                    if let Some(mut task) = self.tasks.get_mut(&span_id) {
                        // In practice, we'd extract error message from event fields
                        task.fail("Task failed".to_string());
                        self.update_display();
                    }
                }
            }
            _ => {
                // For other events, just trigger a display update if we have active tasks
                if !self.tasks.is_empty() {
                    self.update_display();
                }
            }
        }
    }
}

// Implement Drop to ensure final display update
impl Drop for TreeSubscriber {
    fn drop(&mut self) {
        self.final_update();
    }
}
