use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod bridge_layer;
pub mod progress;
pub mod task_span;
pub mod tree_formatter;
pub mod tree_subscriber;

// Re-export tracing macros for convenience
pub use tracing::{debug, error, info, instrument, span, trace, warn, Level, Span};
pub use bridge_layer::EventBridgeLayer;

/// Initialize the tracing system
///
/// Detects TTY vs non-TTY environments and configures the appropriate subscriber.
/// For TTY environments, uses the custom tree subscriber for real-time task visualization.
/// For non-TTY environments, uses a simple formatter that maintains compatibility.
/// Also initializes the event bridge layer for Phase 3 event system integration.
pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    // Initialize the event bridge layer for Phase 3 integration
    let bridge_layer = EventBridgeLayer::new();

    if is_tty() {
        // TTY environment - use tree subscriber for interactive display
        let tree_layer = tree_subscriber::TreeSubscriber::new();

        tracing_subscriber::registry()
            .with(filter)
            .with(bridge_layer)
            .with(tree_layer)
            .try_init()?;
    } else {
        // Non-TTY environment - use simple formatter for compatibility
        let fmt_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .compact()
            .with_target(false)
            .with_thread_ids(false)
            .with_level(true);

        tracing_subscriber::registry()
            .with(filter)
            .with(bridge_layer)
            .with(fmt_layer)
            .try_init()?;
    }

    Ok(())
}

/// Check if we're running in a TTY environment
fn is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}

/// Create a span for task execution with proper metadata
pub fn task_span(name: &str, target: Option<&str>) -> Span {
    span!(Level::INFO, "task", task_name = %name, target = target.unwrap_or(""))
}

/// Create a span for execution level grouping
pub fn level_span(level: usize, total_tasks: usize) -> Span {
    span!(Level::INFO, "level", level = %level, total_tasks = %total_tasks)
}

/// Create a span for the entire pipeline execution
pub fn pipeline_span(total_tasks: usize) -> Span {
    span!(Level::INFO, "pipeline", total_tasks = %total_tasks)
}

/// Emit a structured event for task progress
pub fn task_progress(task_name: &str, progress: Option<u8>, message: &str) {
    if let Some(percent) = progress {
        info!(
            task_name = %task_name,
            progress_percent = %percent,
            message = %message,
            "task_progress"
        );
    } else {
        info!(
            task_name = %task_name,
            message = %message,
            "task_progress"
        );
    }
}

/// Emit a structured event for task completion
pub fn task_completed(task_name: &str, duration_ms: u64, success: bool) {
    if success {
        info!(
            task_name = %task_name,
            duration_ms = %duration_ms,
            "task_completed"
        );
    } else {
        error!(
            task_name = %task_name,
            duration_ms = %duration_ms,
            "task_failed"
        );
    }
}

/// Emit a structured event for cache operations
pub fn cache_event(task_name: &str, hit: bool, operation: &str) {
    if hit {
        debug!(
            task_name = %task_name,
            operation = %operation,
            "cache_hit"
        );
    } else {
        debug!(
            task_name = %task_name,
            operation = %operation,
            "cache_miss"
        );
    }
}
