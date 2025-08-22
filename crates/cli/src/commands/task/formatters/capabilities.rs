//! Terminal capability detection for formatters

use std::io::{self, IsTerminal};

/// Terminal capabilities for choosing appropriate formatter
#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    /// Whether the terminal supports colors
    pub supports_color: bool,
    /// Whether the terminal supports unicode characters
    pub supports_unicode: bool,
    /// Whether the terminal supports cursor movement
    pub supports_cursor_movement: bool,
    /// Terminal width in columns
    pub width: Option<u16>,
    /// Terminal height in rows
    pub height: Option<u16>,
}

impl TerminalCapabilities {
    /// Detect current terminal capabilities
    pub fn detect() -> Self {
        let is_terminal = io::stderr().is_terminal();
        let supports_color = is_terminal && supports_color();
        let supports_unicode = is_terminal && supports_unicode();
        let supports_cursor_movement = is_terminal;

        let (width, height) = if is_terminal {
            terminal_size()
        } else {
            (None, None)
        };

        Self {
            supports_color,
            supports_unicode,
            supports_cursor_movement,
            width,
            height,
        }
    }

    /// Whether this terminal can display rich formatting
    pub fn is_rich(&self) -> bool {
        self.supports_color && self.supports_unicode && self.supports_cursor_movement
    }

    /// Whether this terminal is suitable for interactive display
    pub fn is_interactive(&self) -> bool {
        self.supports_cursor_movement && self.width.is_some()
    }

    /// Recommend the best formatter based on capabilities
    pub fn recommend_formatter(&self, requested: Option<&str>) -> &'static str {
        if let Some(requested) = requested {
            return match requested {
                "simple" => "simple",
                "spinner" => "spinner",
                "tui" => "tui",
                _ => "simple",
            };
        }

        if Self::is_ci_environment() {
            "simple"
        } else if self.is_rich() && self.is_interactive() {
            "tui"
        } else if self.supports_color {
            "spinner"
        } else {
            "simple"
        }
    }

    /// Display terminal capability information
    pub fn display_info(&self) -> String {
        format!(
            "Terminal: {}x{} colors:{} unicode:{} cursor:{}",
            self.width
                .map(|w| w.to_string())
                .unwrap_or_else(|| "?".to_string()),
            self.height
                .map(|h| h.to_string())
                .unwrap_or_else(|| "?".to_string()),
            if self.supports_color { "yes" } else { "no" },
            if self.supports_unicode { "yes" } else { "no" },
            if self.supports_cursor_movement {
                "yes"
            } else {
                "no"
            }
        )
    }

    /// Detect if running in a CI environment
    pub fn is_ci_environment() -> bool {
        // Check common CI environment variables
        std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("GITLAB_CI").is_ok()
            || std::env::var("JENKINS_URL").is_ok()
            || std::env::var("BUILDKITE").is_ok()
            || std::env::var("CIRCLECI").is_ok()
            || std::env::var("TRAVIS").is_ok()
    }
}

fn supports_color() -> bool {
    // Check environment variables that indicate color support
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("color") || term.contains("256") || term == "xterm-kitty" {
            return true;
        }
    }

    if let Ok(colorterm) = std::env::var("COLORTERM") {
        if colorterm == "truecolor" || colorterm == "24bit" {
            return true;
        }
    }

    // Check for common color-supporting terminals
    std::env::var("FORCE_COLOR").is_ok()
        || std::env::var("CLICOLOR_FORCE")
            .map(|v| v != "0")
            .unwrap_or(false)
}

fn supports_unicode() -> bool {
    // Check locale for UTF-8 support
    if let Ok(locale) = std::env::var("LC_ALL").or_else(|_| std::env::var("LC_CTYPE")) {
        return locale.to_uppercase().contains("UTF-8") || locale.to_uppercase().contains("UTF8");
    }

    if let Ok(lang) = std::env::var("LANG") {
        return lang.to_uppercase().contains("UTF-8") || lang.to_uppercase().contains("UTF8");
    }

    // Default to true on common platforms
    cfg!(target_os = "macos") || cfg!(target_os = "linux")
}

fn terminal_size() -> (Option<u16>, Option<u16>) {
    if let Ok((w, h)) = crossterm::terminal::size() {
        (Some(w), Some(h))
    } else {
        (None, None)
    }
}
