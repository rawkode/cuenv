//! Event system for the TUI application
//! Based on bottom's event architecture

use crossterm::event::{KeyEvent, MouseEvent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// Import task-related types for DAG events
use cuenv_core::TaskDefinition;

/// Simplified flattened task for TUI communication
#[derive(Debug, Clone)]
pub struct FlattenedTask {
    pub id: String,
    pub name: String,
    pub group_path: Vec<String>,
    pub dependencies: Vec<String>,
    pub is_barrier: bool,
}

/// Events sent to the main TUI thread
#[derive(Debug)]
pub enum TuiEvent {
    /// Terminal was resized
    Resize,
    /// Keyboard input
    KeyInput(KeyEvent),
    /// Mouse input
    MouseInput(MouseEvent),
    /// Task execution event from tracing
    TaskUpdate(TaskEvent),
    /// General tracing event for tracing pane
    TracingUpdate(TracingEvent),
    /// DAG information sent before execution starts
    DAGReceived {
        tasks: Vec<FlattenedTask>,
        execution_levels: Vec<Vec<String>>,
        task_definitions: HashMap<String, TaskDefinition>,
    },
    /// Signal to terminate the application
    Terminate,
}

/// Task-specific events from the execution system
#[derive(Debug, Clone)]
pub enum TaskEvent {
    Started {
        task_name: String,
        timestamp: Instant,
    },
    Progress {
        task_name: String,
        message: String,
    },
    Completed {
        task_name: String,
        exit_code: i32,
        duration_ms: u64,
    },
    Failed {
        task_name: String,
        error: String,
        duration_ms: u64,
    },
    Cancelled {
        task_name: String,
    },
    Log {
        task_name: String,
        stream: LogStream,
        content: String,
    },
}

/// Log stream types
#[derive(Debug, Clone, PartialEq)]
pub enum LogStream {
    Stdout,
    Stderr,
    System,
}

/// General tracing events for the tracing pane
#[derive(Debug, Clone)]
pub struct TracingEvent {
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub level: TracingLevel,
    pub target: String,
    pub message: String,
    pub fields: Vec<(String, String)>,
}

/// Tracing levels
#[derive(Debug, Clone, PartialEq)]
pub enum TracingLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl TracingLevel {
    pub fn from_tracing_level(level: &tracing::Level) -> Self {
        match *level {
            tracing::Level::TRACE => Self::Trace,
            tracing::Level::DEBUG => Self::Debug,
            tracing::Level::INFO => Self::Info,
            tracing::Level::WARN => Self::Warn,
            tracing::Level::ERROR => Self::Error,
        }
    }

    pub fn style(&self) -> ratatui::style::Style {
        use ratatui::style::{Color, Modifier, Style};
        match self {
            TracingLevel::Trace => Style::default().fg(Color::DarkGray),
            TracingLevel::Debug => Style::default().fg(Color::Blue),
            TracingLevel::Info => Style::default().fg(Color::Green),
            TracingLevel::Warn => Style::default().fg(Color::Yellow),
            TracingLevel::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            TracingLevel::Trace => "TRACE",
            TracingLevel::Debug => "DEBUG",
            TracingLevel::Info => "INFO ",
            TracingLevel::Warn => "WARN ",
            TracingLevel::Error => "ERROR",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TracingLevel::Trace => "▪",
            TracingLevel::Debug => "🔍",
            TracingLevel::Info => "ℹ",
            TracingLevel::Warn => "⚠",
            TracingLevel::Error => "❌",
        }
    }
}

/// Events sent to control threads
#[derive(Debug)]
pub enum ControlEvent {
    /// Reset/restart task execution
    Reset,
    /// Pause task execution
    Pause,
    /// Resume task execution
    Resume,
}

/// Task states for the TUI display
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Queued => "◌",
            Self::Running => "▣",
            Self::Completed => "✓",
            Self::Failed => "✖",
            Self::Cancelled => "⊘",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// Log entry for task output
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: Instant,
    pub stream: LogStream,
    pub content: String,
}

/// Task information for display
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub name: String,
    pub state: TaskState,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
    pub logs: Vec<LogEntry>,
}

impl TaskInfo {
    pub fn new(name: String, dependencies: Vec<String>) -> Self {
        Self {
            name,
            state: TaskState::Queued,
            dependencies,
            dependents: Vec::new(),
            start_time: None,
            end_time: None,
            exit_code: None,
            message: None,
            logs: Vec::new(),
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            (Some(start), None) => Some(Instant::now().duration_since(start)),
            _ => None,
        }
    }

    /// Get duration as of a specific time (for consistent display)
    pub fn duration_at(&self, now: Instant) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            (Some(start), None) => Some(now.duration_since(start)),
            _ => None,
        }
    }
}

/// Task registry for managing task state
#[derive(Clone)]
pub struct TaskRegistry {
    tasks: Arc<RwLock<HashMap<String, TaskInfo>>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_task(&self, name: String, dependencies: Vec<String>) {
        let mut tasks = self.tasks.write().await;
        let task = TaskInfo::new(name.clone(), dependencies);

        // Update dependents for each dependency
        for dep in &task.dependencies {
            if let Some(dep_task) = tasks.get_mut(dep) {
                dep_task.dependents.push(name.clone());
            }
        }

        tasks.insert(name, task);
    }

    pub async fn update_task_state(&self, name: &str, state: TaskState) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(name) {
            task.state = state;
            match &task.state {
                TaskState::Running => {
                    task.start_time = Some(Instant::now());
                }
                TaskState::Completed | TaskState::Failed | TaskState::Cancelled => {
                    task.end_time = Some(Instant::now());
                }
                _ => {}
            }
        }
    }

    pub async fn add_log(&self, name: &str, stream: LogStream, content: String) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(name) {
            task.logs.push(LogEntry {
                timestamp: Instant::now(),
                stream,
                content,
            });
        }
    }

    pub async fn update_progress(&self, name: &str, message: String) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(name) {
            task.message = Some(message);
        }
    }

    pub async fn set_exit_code(&self, name: &str, exit_code: i32) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(name) {
            task.exit_code = Some(exit_code);
        }
    }

    pub async fn get_task(&self, name: &str) -> Option<TaskInfo> {
        let tasks = self.tasks.read().await;
        tasks.get(name).cloned()
    }

    pub async fn get_all_tasks(&self) -> HashMap<String, TaskInfo> {
        let tasks = self.tasks.read().await;
        tasks.clone()
    }

    pub async fn get_aggregate_state(&self, task_name: &str) -> TaskState {
        let tasks = self.tasks.read().await;
        Self::aggregate_state_recursive(task_name, &tasks)
    }

    fn aggregate_state_recursive(task_name: &str, tasks: &HashMap<String, TaskInfo>) -> TaskState {
        if let Some(task) = tasks.get(task_name) {
            // If any dependent has failed, this task's aggregate state is failed
            for dep in &task.dependents {
                let dep_state = Self::aggregate_state_recursive(dep, tasks);
                if dep_state == TaskState::Failed {
                    return TaskState::Failed;
                }
            }
            task.state.clone()
        } else {
            TaskState::Queued
        }
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}
