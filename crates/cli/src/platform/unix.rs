#[cfg(test)]
use super::{escape_shell_value, ExportFormat};
use super::{PlatformOps, Shell};
use std::collections::HashMap;
use std::env;

pub struct UnixPlatform;

impl PlatformOps for UnixPlatform {
    fn get_current_shell() -> Result<Shell, String> {
        // First try the SHELL environment variable
        if let Ok(shell_path) = env::var("SHELL") {
            if let Some(shell_name) = shell_path.split('/').next_back() {
                if let Ok(shell) = shell_name.parse::<Shell>() {
                    if shell.is_unix() {
                        return Ok(shell);
                    }
                }
            }
        }

        // Fallback for fish shell
        if env::var("FISH_VERSION").is_ok() {
            return Ok(Shell::Fish);
        }

        // Try to detect from parent process
        if let Ok(ppid) = std::fs::read_to_string("/proc/self/stat") {
            if let Some(ppid_str) = ppid.split_whitespace().nth(3) {
                if let Ok(ppid_num) = ppid_str.parse::<u32>() {
                    if let Ok(cmd) = std::fs::read_to_string(format!("/proc/{ppid_num}/comm")) {
                        let shell_name = cmd.trim();
                        if let Ok(shell) = shell_name.parse::<Shell>() {
                            if shell.is_unix() {
                                return Ok(shell);
                            }
                        }
                    }
                }
            }
        }

        // Default to bash on Unix
        Ok(Shell::Bash)
    }

    #[cfg(test)]
    fn _get_export_format(shell: Shell) -> ExportFormat {
        match shell {
            Shell::Bash | Shell::Zsh => {
                ExportFormat::new("export ", "=", "", "unset ", escape_shell_value)
            }
            Shell::Fish => ExportFormat::new("set -x ", " ", "", "set -e ", escape_shell_value),
            _ => {
                // Shouldn't happen on Unix, but provide bash format as fallback
                ExportFormat::new("export ", "=", "", "unset ", escape_shell_value)
            }
        }
    }

    fn _home_env_var() -> &'static str {
        "HOME"
    }

    fn _setup_environment(_env: &mut HashMap<String, String>) {
        // Unix doesn't need special environment setup
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_shell_detection() {
        // Test shell detection logic and field access
        let format = UnixPlatform::_get_export_format(Shell::Bash);
        assert_eq!(format.prefix(), "export ");
        assert_eq!(format.separator(), "=");
        assert_eq!(format.suffix(), "");
        assert_eq!(format.unset_prefix(), "unset ");
        // Test escape_value function pointer
        assert_eq!((format.escape_value())("test"), "'test'");

        let format = UnixPlatform::_get_export_format(Shell::Fish);
        assert_eq!(format.prefix(), "set -x ");
        assert_eq!(format.separator(), " ");
        assert_eq!(format.suffix(), "");
        assert_eq!(format.unset_prefix(), "set -e ");
        // Test escape_value function pointer
        assert_eq!((format.escape_value())("test"), "'test'");
    }

    #[test]
    fn test_unix_export_format() {
        let format = UnixPlatform::_get_export_format(Shell::Bash);
        let export = format._format_export("KEY", "value with spaces");
        assert_eq!(export, "export KEY='value with spaces'");

        let export = format._format_export("KEY", "value with 'quotes'");
        assert_eq!(export, "export KEY='value with '\\''quotes'\\'''");

        let unset = format._format_unset("KEY");
        assert_eq!(unset, "unset KEY");

        // Verify field values are accessible
        let (prefix, sep, suffix, unset_prefix) = format.field_values();
        assert_eq!(prefix, "export ");
        assert_eq!(sep, "=");
        assert_eq!(suffix, "");
        assert_eq!(unset_prefix, "unset ");

        // Verify escape function works
        let escape_fn = format.get_escape_fn();
        assert_eq!(escape_fn("test"), "'test'");
    }

    #[test]
    fn test_fish_export_format() {
        let format = UnixPlatform::_get_export_format(Shell::Fish);
        let export = format._format_export("KEY", "value");
        assert_eq!(export, "set -x KEY 'value'");

        let unset = format._format_unset("KEY");
        assert_eq!(unset, "set -e KEY");
    }
}
