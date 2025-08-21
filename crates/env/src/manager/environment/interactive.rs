use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal,
};
use cuenv_utils::hooks_status::HooksStatusManager;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Represents the result of an interactive monitoring operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlow {
    /// The operation should continue in the foreground.
    Continue,
    /// The user requested to abort the operation (q key).
    Aborted,
    /// The operation was interrupted (Ctrl+C).
    Interrupted,
}

/// Handles interactive terminal operations, such as monitoring for user input
/// with a timeout and displaying progress.
pub struct InteractiveHandler {
    start_time: Instant,
    last_progress_update: Instant,
}

impl InteractiveHandler {
    /// Creates a new `InteractiveHandler`.
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            last_progress_update: Instant::now(),
        }
    }

    /// Creates a new `InteractiveHandler` with a status manager for progress tracking.
    pub fn with_status_manager(_status_manager: Arc<HooksStatusManager>) -> Self {
        Self {
            start_time: Instant::now(),
            last_progress_update: Instant::now(),
        }
    }

    /// Monitors for user input with a specified timeout.
    ///
    /// This function shows a progress indicator and listens for user input
    /// to quit the task. Used only for interactive commands like 'cuenv task' and 'cuenv exec'.
    pub async fn monitor_with_timeout(&mut self, duration: Duration) -> ControlFlow {
        // Update progress display if enough time has passed
        if self.last_progress_update.elapsed() > Duration::from_millis(500) {
            self.display_progress();
            self.last_progress_update = Instant::now();
        }

        // Don't enable raw mode for short durations to avoid terminal flicker
        if duration < Duration::from_millis(100) {
            tokio::time::sleep(duration).await;
            return ControlFlow::Continue;
        }

        // Try to enable raw mode, but don't fail if we can't
        let raw_mode_enabled = terminal::enable_raw_mode().is_ok();

        let result = if raw_mode_enabled {
            self.poll_for_input(duration).await
        } else {
            // If we can't enable raw mode, just wait without polling for input
            tokio::time::sleep(duration).await;
            ControlFlow::Continue
        };

        if raw_mode_enabled {
            let _ = terminal::disable_raw_mode();
        }

        result
    }

    fn display_progress(&self) {
        let elapsed = self.start_time.elapsed();
        let dots = (elapsed.as_secs() % 4) as usize;
        let progress_indicator = ".".repeat(dots + 1);

        // Clear line and display progress
        tracing::error!(
            "\r# cuenv: Running hooks{:<4} ({}s elapsed)",
            progress_indicator,
            elapsed.as_secs()
        );

        // Flush to ensure immediate display
        let _ = io::stderr().flush();
    }

    async fn poll_for_input(&self, duration: Duration) -> ControlFlow {
        // Use a shorter poll duration to be more responsive
        let poll_duration = duration.min(Duration::from_millis(50));

        // Poll for events in a loop to check multiple times during the duration
        let start = std::time::Instant::now();
        while start.elapsed() < duration {
            if event::poll(poll_duration).unwrap_or(false) {
                match event::read() {
                    Ok(Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    })) => {
                        tracing::error!("\r\x1b[K# cuenv: Aborting...");
                        return ControlFlow::Aborted;
                    }
                    Ok(Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    })) => {
                        tracing::error!("\r\x1b[K# cuenv: Interrupted!");
                        return ControlFlow::Interrupted;
                    }
                    _ => {}
                }
            }

            // Small async sleep to yield control
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        ControlFlow::Continue
    }
}

impl Default for InteractiveHandler {
    fn default() -> Self {
        Self::new()
    }
}
