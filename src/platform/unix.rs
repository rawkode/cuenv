use super::{escape_shell_value, ExportFormat, PlatformOps, Shell};
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

    fn get_export_format(shell: Shell) -> ExportFormat {
        match shell {
            Shell::Bash | Shell::Zsh => ExportFormat {
                prefix: "export ",
                separator: "=",
                suffix: "",
                unset_prefix: "unset ",
                escape_value: escape_shell_value,
            },
            Shell::Fish => ExportFormat {
                prefix: "set -x ",
                separator: " ",
                suffix: "",
                unset_prefix: "set -e ",
                escape_value: escape_shell_value,
            },
            _ => {
                // Shouldn't happen on Unix, but provide bash format as fallback
                ExportFormat {
                    prefix: "export ",
                    separator: "=",
                    suffix: "",
                    unset_prefix: "unset ",
                    escape_value: escape_shell_value,
                }
            }
        }
    }

    fn home_env_var() -> &'static str {
        "HOME"
    }

    fn setup_environment(_env: &mut HashMap<String, String>) {
        // Unix doesn't need special environment setup
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_shell_detection() {
        // Test shell detection logic
        let format = UnixPlatform::get_export_format(Shell::Bash);
        assert_eq!(format.prefix, "export ");
        assert_eq!(format.separator, "=");

        let format = UnixPlatform::get_export_format(Shell::Fish);
        assert_eq!(format.prefix, "set -x ");
        assert_eq!(format.separator, " ");
    }

    #[test]
    fn test_unix_export_format() {
        let format = UnixPlatform::get_export_format(Shell::Bash);
        let export = format.format_export("KEY", "value with spaces");
        assert_eq!(export, "export KEY='value with spaces'");

        let export = format.format_export("KEY", "value with 'quotes'");
        assert_eq!(export, "export KEY='value with '\\''quotes'\\'''");

        let unset = format.format_unset("KEY");
        assert_eq!(unset, "unset KEY");
    }

    #[test]
    fn test_fish_export_format() {
        let format = UnixPlatform::get_export_format(Shell::Fish);
        let export = format.format_export("KEY", "value");
        assert_eq!(export, "set -x KEY 'value'");

        let unset = format.format_unset("KEY");
        assert_eq!(unset, "set -e KEY");
    }
}
