//! Terminal management utilities
//! Based on bottom's terminal handling

use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout, Write};
use std::panic::{self, PanicHookInfo};

/// Terminal manager that handles setup and cleanup
pub struct TerminalManager {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
    mouse_capture_enabled: bool,
}

impl TerminalManager {
    /// Create a new terminal manager and set up the terminal
    pub fn new() -> io::Result<Self> {
        // Check if we're in a terminal
        check_if_terminal();

        // Enable raw mode
        enable_raw_mode().map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to enable raw mode: {}", e),
            )
        })?;

        let mut stdout = io::stdout();

        // Enter alternate screen
        execute!(stdout, Hide, EnterAlternateScreen).map_err(|e| {
            let _ = disable_raw_mode();
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to enter alternate screen: {}", e),
            )
        })?;

        // Enable mouse capture
        execute!(stdout, EnableMouseCapture).map_err(|e| {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to enable mouse capture: {}", e),
            )
        })?;

        // Create terminal
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).map_err(|e| {
            let _ = execute!(
                io::stdout(),
                DisableMouseCapture,
                LeaveAlternateScreen,
                Show
            );
            let _ = disable_raw_mode();
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to create terminal: {}", e),
            )
        })?;

        terminal.clear().map_err(|e| {
            let _ = execute!(
                io::stdout(),
                DisableMouseCapture,
                LeaveAlternateScreen,
                Show
            );
            let _ = disable_raw_mode();
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to clear terminal: {}", e),
            )
        })?;

        terminal.hide_cursor().map_err(|e| {
            let _ = execute!(
                io::stdout(),
                DisableMouseCapture,
                LeaveAlternateScreen,
                Show
            );
            let _ = disable_raw_mode();
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to hide cursor: {}", e),
            )
        })?;

        Ok(Self {
            terminal,
            raw_mode_enabled: true,
            alternate_screen_enabled: true,
            mouse_capture_enabled: true,
        })
    }

    /// Get a mutable reference to the terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Clean up the terminal and restore it to normal state
    pub fn cleanup(&mut self) -> io::Result<()> {
        if self.mouse_capture_enabled {
            execute!(self.terminal.backend_mut(), DisableMouseCapture)?;
            self.mouse_capture_enabled = false;
        }

        if self.alternate_screen_enabled {
            execute!(
                self.terminal.backend_mut(),
                LeaveAlternateScreen,
                Clear(ClearType::All),
                Show
            )?;
            self.alternate_screen_enabled = false;
        }

        self.terminal.show_cursor()?;

        if self.raw_mode_enabled {
            disable_raw_mode()?;
            self.raw_mode_enabled = false;
        }

        // Clear any remaining output and flush
        self.terminal.backend_mut().flush()?;

        Ok(())
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Check if the current environment is in a terminal and warn if not
fn check_if_terminal() {
    use crossterm::tty::IsTty;

    if !io::stdout().is_tty() {
        tracing::warn!(
            "cuenv TUI is not being output to a terminal. Things might not work properly."
        );
        tracing::warn!("If you're stuck, press 'q' or 'Ctrl-c' to quit the program.");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Reset stdout back to normal state (for panic recovery)
pub fn reset_stdout() {
    let mut stdout = io::stdout();
    let _ = disable_raw_mode();
    let _ = execute!(
        stdout,
        DisableMouseCapture,
        LeaveAlternateScreen,
        Clear(ClearType::All),
        Show
    );
    let _ = stdout.flush();
}

/// Panic hook to properly restore the terminal
pub fn create_panic_hook() -> Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync> {
    Box::new(|panic_info: &PanicHookInfo<'_>| {
        let msg = match panic_info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        // Restore terminal first
        reset_stdout();

        // Print panic information
        if let Some(location) = panic_info.location() {
            tracing::error!("thread 'main' panicked at '{msg}', {location}");
        } else {
            tracing::error!("thread 'main' panicked at '{msg}'");
        }

        // Exit immediately
        std::process::exit(1);
    })
}

/// Set up the panic hook
pub fn setup_panic_hook() {
    panic::set_hook(create_panic_hook());
}
