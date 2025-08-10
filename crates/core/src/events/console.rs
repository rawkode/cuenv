//! Console event subscriber for terminal output

use crate::events::{EnhancedEvent, EventSubscriber, SystemEvent, TaskEvent, PipelineEvent, CacheEvent};
use async_trait::async_trait;
use std::io::{self};
use tracing::{debug};

/// Console subscriber for terminal output
pub struct ConsoleSubscriber {
    /// Use colored output
    use_colors: bool,
    /// Verbosity level
    verbosity: ConsoleVerbosity,
    /// Output writer
    writer: ConsoleWriter,
}

/// Console verbosity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleVerbosity {
    /// Only show errors and critical events
    Quiet,
    /// Show important events (default)
    Normal,
    /// Show all events including progress
    Verbose,
    /// Show debug information
    Debug,
}

/// Console output destination
enum ConsoleWriter {
    Stderr,
    Stdout,
}

impl ConsoleSubscriber {
    /// Create a new console subscriber with default settings
    pub fn new() -> Self {
        Self {
            use_colors: io::IsTerminal::is_terminal(&io::stderr()),
            verbosity: ConsoleVerbosity::Normal,
            writer: ConsoleWriter::Stderr,
        }
    }

    /// Create a console subscriber with custom settings
    pub fn with_config(use_colors: bool, verbosity: ConsoleVerbosity) -> Self {
        Self {
            use_colors,
            verbosity,
            writer: ConsoleWriter::Stderr,
        }
    }

    /// Create a console subscriber for CI/CD environments
    pub fn for_ci() -> Self {
        Self {
            use_colors: false,
            verbosity: ConsoleVerbosity::Normal,
            writer: ConsoleWriter::Stdout,
        }
    }

    /// Format an event for console output
    fn format_event(&self, event: &EnhancedEvent) -> Option<String> {
        match &event.event {
            SystemEvent::Task(task_event) => self.format_task_event(task_event),
            SystemEvent::Pipeline(pipeline_event) => self.format_pipeline_event(pipeline_event),
            SystemEvent::Cache(cache_event) => self.format_cache_event(cache_event),
            SystemEvent::Env(env_event) => {
                if matches!(self.verbosity, ConsoleVerbosity::Verbose | ConsoleVerbosity::Debug) {
                    Some(format!("🌍 {}", self.format_env_event(env_event)))
                } else {
                    None
                }
            }
            SystemEvent::Dependency(dep_event) => {
                if matches!(self.verbosity, ConsoleVerbosity::Debug) {
                    Some(format!("🔗 {}", self.format_dependency_event(dep_event)))
                } else {
                    None
                }
            }
        }
    }

    fn format_task_event(&self, event: &TaskEvent) -> Option<String> {
        match event {
            TaskEvent::TaskStarted { task_name, .. } => {
                if matches!(
                    self.verbosity,
                    ConsoleVerbosity::Verbose | ConsoleVerbosity::Debug
                ) {
                    Some(self.colorize(&format!("▶ Starting task '{}'", task_name), "blue"))
                } else {
                    None
                }
            }
            TaskEvent::TaskCompleted {
                task_name,
                duration_ms,
                ..
            } => Some(self.colorize(
                &format!("✅ Task '{}' completed in {}ms", task_name, duration_ms),
                "green",
            )),
            TaskEvent::TaskFailed {
                task_name, error, ..
            } => Some(self.colorize(
                &format!("❌ Task '{}' failed: {}", task_name, error),
                "red",
            )),
            TaskEvent::TaskProgress { task_name, message, .. } => {
                if matches!(
                    self.verbosity,
                    ConsoleVerbosity::Verbose | ConsoleVerbosity::Debug
                ) {
                    Some(self.colorize(&format!("⏳ {}: {}", task_name, message), "yellow"))
                } else {
                    None
                }
            }
            TaskEvent::TaskSkipped {
                task_name, reason, ..
            } => {
                if matches!(
                    self.verbosity,
                    ConsoleVerbosity::Verbose | ConsoleVerbosity::Debug
                ) {
                    Some(self.colorize(
                        &format!("⏭ Task '{}' skipped: {}", task_name, reason),
                        "cyan",
                    ))
                } else {
                    None
                }
            }
            TaskEvent::TaskOutput { task_name, output, .. } => {
                if matches!(self.verbosity, ConsoleVerbosity::Debug) {
                    Some(format!("📤 {}: {}", task_name, output))
                } else {
                    None
                }
            }
            TaskEvent::TaskError { task_name, error, .. } => {
                Some(self.colorize(&format!("🚨 {}: {}", task_name, error), "red"))
            }
        }
    }

    fn format_pipeline_event(&self, event: &PipelineEvent) -> Option<String> {
        match event {
            PipelineEvent::PipelineStarted {
                total_tasks,
                total_levels,
            } => Some(self.colorize(
                &format!(
                    "🚀 Starting pipeline: {} tasks across {} levels",
                    total_tasks, total_levels
                ),
                "blue",
            )),
            PipelineEvent::LevelStarted {
                level,
                tasks_in_level,
            } => {
                if matches!(
                    self.verbosity,
                    ConsoleVerbosity::Verbose | ConsoleVerbosity::Debug
                ) {
                    Some(self.colorize(
                        &format!("📊 Level {}: {} tasks", level, tasks_in_level),
                        "cyan",
                    ))
                } else {
                    None
                }
            }
            PipelineEvent::LevelCompleted {
                level,
                successful_tasks,
                failed_tasks,
            } => {
                if *failed_tasks > 0 {
                    Some(self.colorize(
                        &format!(
                            "📊 Level {} completed: {} successful, {} failed",
                            level, successful_tasks, failed_tasks
                        ),
                        "yellow",
                    ))
                } else if matches!(
                    self.verbosity,
                    ConsoleVerbosity::Verbose | ConsoleVerbosity::Debug
                ) {
                    Some(self.colorize(
                        &format!("📊 Level {} completed: {} tasks successful", level, successful_tasks),
                        "green",
                    ))
                } else {
                    None
                }
            }
            PipelineEvent::PipelineCompleted {
                total_duration_ms,
                successful_tasks,
                failed_tasks,
            } => Some(self.colorize(
                &format!(
                    "🏁 Pipeline completed in {}ms: {} successful, {} failed",
                    total_duration_ms, successful_tasks, failed_tasks
                ),
                if *failed_tasks > 0 { "red" } else { "green" },
            )),
        }
    }

    fn format_cache_event(&self, event: &CacheEvent) -> Option<String> {
        if !matches!(
            self.verbosity,
            ConsoleVerbosity::Verbose | ConsoleVerbosity::Debug
        ) {
            return None;
        }

        match event {
            CacheEvent::CacheHit { key } => {
                Some(self.colorize(&format!("💾 Cache hit: {}", key), "green"))
            }
            CacheEvent::CacheMiss { key } => {
                Some(self.colorize(&format!("💿 Cache miss: {}", key), "yellow"))
            }
            CacheEvent::CacheWrite { key, size_bytes } => {
                Some(self.colorize(
                    &format!("💾 Cache write: {} ({} bytes)", key, size_bytes),
                    "cyan",
                ))
            }
            CacheEvent::CacheEvict { key, reason } => {
                Some(self.colorize(
                    &format!("🗑 Cache evict: {} ({})", key, reason),
                    "red",
                ))
            }
        }
    }

    fn format_env_event(&self, event: &crate::events::EnvEvent) -> String {
        match event {
            crate::events::EnvEvent::EnvLoading { path } => {
                format!("Loading environment from {}", path)
            }
            crate::events::EnvEvent::EnvLoaded { path, var_count } => {
                format!("Loaded {} variables from {}", var_count, path)
            }
            crate::events::EnvEvent::EnvLoadFailed { path, error } => {
                format!("Failed to load environment from {}: {}", path, error)
            }
            crate::events::EnvEvent::EnvVarChanged { key, is_secret } => {
                if *is_secret {
                    format!("Environment variable {} changed (sensitive)", key)
                } else {
                    format!("Environment variable {} changed", key)
                }
            }
        }
    }

    fn format_dependency_event(&self, event: &crate::events::DependencyEvent) -> String {
        match event {
            crate::events::DependencyEvent::DependencyResolved {
                task_name,
                dependency_name,
                package_name,
            } => {
                if let Some(pkg) = package_name {
                    format!(
                        "Task '{}' resolved dependency '{}' from package '{}'",
                        task_name, dependency_name, pkg
                    )
                } else {
                    format!(
                        "Task '{}' resolved dependency '{}'",
                        task_name, dependency_name
                    )
                }
            }
            crate::events::DependencyEvent::DependencyResolutionFailed {
                task_name,
                dependency_name,
                error,
            } => {
                format!(
                    "Task '{}' failed to resolve dependency '{}': {}",
                    task_name, dependency_name, error
                )
            }
        }
    }

    /// Apply color to text if colors are enabled
    fn colorize(&self, text: &str, color: &str) -> String {
        if !self.use_colors {
            return text.to_string();
        }

        let color_code = match color {
            "red" => "\x1b[31m",
            "green" => "\x1b[32m",
            "yellow" => "\x1b[33m",
            "blue" => "\x1b[34m",
            "magenta" => "\x1b[35m",
            "cyan" => "\x1b[36m",
            "white" => "\x1b[37m",
            _ => "\x1b[0m",
        };

        format!("{}{}\x1b[0m", color_code, text)
    }

    /// Write output to the configured destination
    fn write_output(&self, content: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self.writer {
            ConsoleWriter::Stderr => {
                eprintln!("{}", content);
            }
            ConsoleWriter::Stdout => {
                println!("{}", content);
            }
        }
        Ok(())
    }
}

impl Default for ConsoleSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventSubscriber for ConsoleSubscriber {
    async fn handle_event(&self, event: &EnhancedEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check verbosity level for early filtering
        match (&event.event, self.verbosity) {
            // Always show errors and completed tasks
            (SystemEvent::Task(TaskEvent::TaskFailed { .. }), _) => {}
            (SystemEvent::Task(TaskEvent::TaskCompleted { .. }), _) => {}
            (SystemEvent::Pipeline(PipelineEvent::PipelineCompleted { .. }), _) => {}
            
            // Show pipeline start in normal mode
            (SystemEvent::Pipeline(PipelineEvent::PipelineStarted { .. }), ConsoleVerbosity::Normal | ConsoleVerbosity::Verbose | ConsoleVerbosity::Debug) => {}
            
            // Skip most events in quiet mode
            (_, ConsoleVerbosity::Quiet) => return Ok(()),
            
            // Continue with normal filtering for other cases
            _ => {}
        }

        if let Some(formatted) = self.format_event(event) {
            self.write_output(&formatted)?;
            debug!(
                event_type = std::any::type_name_of_val(&event.event),
                "Console event output"
            );
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "console"
    }

    fn is_interested(&self, event: &SystemEvent) -> bool {
        // Console subscriber is interested in all events, but filters by verbosity
        match (event, self.verbosity) {
            // Always interested in important events
            (SystemEvent::Task(TaskEvent::TaskFailed { .. }), _) => true,
            (SystemEvent::Task(TaskEvent::TaskCompleted { .. }), _) => true,
            (SystemEvent::Task(TaskEvent::TaskError { .. }), _) => true,
            (SystemEvent::Pipeline(PipelineEvent::PipelineCompleted { .. }), _) => true,
            
            // In quiet mode, only show critical events
            (_, ConsoleVerbosity::Quiet) => false,
            
            // Otherwise, all events are potentially interesting
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{SystemEvent, TaskEvent};
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_console_subscriber_task_events() {
        let subscriber = ConsoleSubscriber::with_config(false, ConsoleVerbosity::Verbose);
        
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskStarted {
                task_name: "test".to_string(),
                task_id: "test-1".to_string(),
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: std::collections::HashMap::new(),
        };

        // Should not panic
        let result = subscriber.handle_event(&event).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_console_subscriber_interest_filter() {
        let quiet_subscriber = ConsoleSubscriber::with_config(false, ConsoleVerbosity::Quiet);
        let verbose_subscriber = ConsoleSubscriber::with_config(false, ConsoleVerbosity::Verbose);
        
        let failed_event = SystemEvent::Task(TaskEvent::TaskFailed {
            task_name: "test".to_string(),
            task_id: "test-1".to_string(),
            error: "failed".to_string(),
        });
        
        let progress_event = SystemEvent::Task(TaskEvent::TaskProgress {
            task_name: "test".to_string(),
            task_id: "test-1".to_string(),
            message: "progress".to_string(),
        });

        // Quiet subscriber should only be interested in failed events
        assert!(quiet_subscriber.is_interested(&failed_event));
        assert!(!quiet_subscriber.is_interested(&progress_event));
        
        // Verbose subscriber should be interested in all events
        assert!(verbose_subscriber.is_interested(&failed_event));
        assert!(verbose_subscriber.is_interested(&progress_event));
    }

    #[test]
    fn test_colorize() {
        let color_subscriber = ConsoleSubscriber::with_config(true, ConsoleVerbosity::Normal);
        let no_color_subscriber = ConsoleSubscriber::with_config(false, ConsoleVerbosity::Normal);
        
        let text = "test text";
        
        let colored = color_subscriber.colorize(text, "red");
        assert!(colored.contains("\x1b[31m"));
        assert!(colored.contains("\x1b[0m"));
        
        let uncolored = no_color_subscriber.colorize(text, "red");
        assert_eq!(uncolored, text);
    }
}