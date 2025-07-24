#[cfg(unix)]
mod unix;
/// Platform-specific functionality abstraction
/// This module provides a unified interface for platform-specific operations

#[cfg(windows)]
mod windows;

// Re-export the platform-specific implementation
#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

/// Shell type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
}

impl std::str::FromStr for Shell {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            "fish" => Ok(Self::Fish),
            "powershell" | "pwsh" => Ok(Self::PowerShell),
            "cmd" => Ok(Self::Cmd),
            _ => Err(format!("Unknown shell: {s}")),
        }
    }
}

impl Shell {
    /// Get the shell name as a string
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::PowerShell => "powershell",
            Self::Cmd => "cmd",
        }
    }

    /// Check if this is a Unix shell
    pub const fn is_unix(&self) -> bool {
        matches!(self, Self::Bash | Self::Zsh | Self::Fish)
    }

    /// Check if this is a Windows shell
    pub const fn is_windows(&self) -> bool {
        matches!(self, Self::PowerShell | Self::Cmd)
    }
}

/// Platform-specific operations trait
pub trait PlatformOps {
    /// Get the current shell
    fn get_current_shell() -> Result<Shell, String>;

    /// Get shell-specific export command format
    fn get_export_format(shell: Shell) -> ExportFormat;

    /// Get the user's home directory environment variable name
    fn home_env_var() -> &'static str;

    /// Get platform-specific environment setup
    fn setup_environment(env: &mut std::collections::HashMap<String, String>);
}

/// Export format for different shells
pub struct ExportFormat {
    pub prefix: &'static str,
    pub separator: &'static str,
    pub suffix: &'static str,
    pub unset_prefix: &'static str,
    pub escape_value: fn(&str) -> String,
}

impl ExportFormat {
    /// Format an export command
    pub fn format_export(&self, key: &str, value: &str) -> String {
        format!(
            "{}{}{}{}{}",
            self.prefix,
            key,
            self.separator,
            (self.escape_value)(value),
            self.suffix
        )
    }

    /// Format an unset command
    pub fn format_unset(&self, key: &str) -> String {
        format!("{}{}{}", self.unset_prefix, key, self.suffix)
    }
}

/// Standard shell value escaping for Unix shells
pub fn escape_shell_value(value: &str) -> String {
    // Escape single quotes by replacing ' with '\''
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// PowerShell value escaping
pub fn escape_powershell_value(value: &str) -> String {
    // PowerShell uses backticks for escaping
    format!("'{}'", value.replace('\'', "''"))
}

/// CMD value escaping
pub fn escape_cmd_value(value: &str) -> String {
    // CMD uses double quotes and escapes with ^
    format!("\"{}\"", value.replace('"', "^\""))
}
