// Temporary stubs until dependencies are properly resolved
use cuenv_core::Result;
use std::collections::HashMap;
use std::process::Command;

// Temporary stub for AccessRestrictions until security crate is fixed
#[derive(Default)]
pub struct AccessRestrictions {
    pub file_restrictions: bool,
    pub network_restrictions: bool,
}

impl AccessRestrictions {
    pub fn new(file_restrictions: bool, network_restrictions: bool) -> Self {
        Self {
            file_restrictions,
            network_restrictions,
        }
    }

    pub fn has_any_restrictions(&self) -> bool {
        self.file_restrictions || self.network_restrictions
    }

    pub fn apply_to_command(&self, _cmd: &mut Command) -> Result<()> {
        // Stub - would normally apply Landlock restrictions
        Ok(())
    }
}

// Stubs for missing types
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

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
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

pub struct StateManager;
impl StateManager {
    pub fn load(_path: &std::path::Path) -> Result<()> {
        Ok(())
    }
}

pub struct Platform;

pub struct ExportFormat {
    shell: Shell,
}

impl ExportFormat {
    pub fn new(shell: Shell) -> Self {
        Self { shell }
    }

    pub fn format_export(&self, key: &str, value: &str) -> String {
        match self.shell {
            Shell::Fish => format!("set -x {key} \"{value}\""),
            _ => format!("export {key}=\"{value}\""),
        }
    }

    pub fn format_unset(&self, key: &str) -> String {
        match self.shell {
            Shell::Fish => format!("set -e {key}"),
            _ => format!("unset {key}"),
        }
    }
}

impl Platform {
    pub fn get_export_format(shell: Shell) -> ExportFormat {
        ExportFormat::new(shell)
    }

    pub fn setup_environment(_env: &mut HashMap<String, String>) {
        // Stub
    }

    pub fn home_env_var() -> &'static str {
        "HOME"
    }
}

pub struct OutputFilter<W> {
    writer: W,
}

impl<W: std::io::Write> OutputFilter<W> {
    pub fn new(
        writer: W,
        _secrets: std::sync::Arc<std::sync::RwLock<std::collections::HashSet<String>>>,
    ) -> Self {
        Self { writer }
    }
}

impl<W: std::io::Write> std::io::Write for OutputFilter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
