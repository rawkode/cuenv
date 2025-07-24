use super::{ExportFormat, PlatformOps, Shell, escape_cmd_value, escape_powershell_value};
use std::collections::HashMap;
use std::env;

pub struct WindowsPlatform;

impl PlatformOps for WindowsPlatform {
    fn get_current_shell() -> Result<Shell, String> {
        // Check if we're in PowerShell
        if env::var("PSModulePath").is_ok() {
            return Ok(Shell::PowerShell);
        }

        // Check parent process name
        if let Ok(parent) = env::var("COMSPEC") {
            if parent.to_lowercase().contains("powershell")
                || parent.to_lowercase().contains("pwsh")
            {
                return Ok(Shell::PowerShell);
            }
        }

        // Default to CMD on Windows
        Ok(Shell::Cmd)
    }

    fn get_export_format(shell: Shell) -> ExportFormat {
        match shell {
            Shell::PowerShell => ExportFormat {
                prefix: "$env:",
                separator: " = ",
                suffix: "",
                unset_prefix: "Remove-Item Env:\\",
                escape_value: escape_powershell_value,
            },
            Shell::Cmd => ExportFormat {
                prefix: "set ",
                separator: "=",
                suffix: "",
                unset_prefix: "set ",
                escape_value: escape_cmd_value,
            },
            _ => {
                // Unix shells on Windows (e.g., Git Bash)
                ExportFormat {
                    prefix: "export ",
                    separator: "=",
                    suffix: "",
                    unset_prefix: "unset ",
                    escape_value: super::escape_shell_value,
                }
            }
        }
    }

    fn home_env_var() -> &'static str {
        "USERPROFILE"
    }

    fn setup_environment(env: &mut HashMap<String, String>) {
        // On Windows, if USERPROFILE is set but HOME is not, set HOME to USERPROFILE
        if let Ok(userprofile) = env::var("USERPROFILE") {
            if !env.contains_key("HOME") {
                env.insert("HOME".to_string(), userprofile);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_shell_detection() {
        // Test export formats
        let format = WindowsPlatform::get_export_format(Shell::PowerShell);
        assert_eq!(format.prefix, "$env:");
        assert_eq!(format.separator, " = ");

        let format = WindowsPlatform::get_export_format(Shell::Cmd);
        assert_eq!(format.prefix, "set ");
        assert_eq!(format.separator, "=");
    }

    #[test]
    fn test_powershell_export_format() {
        let format = WindowsPlatform::get_export_format(Shell::PowerShell);
        let export = format.format_export("KEY", "value with spaces");
        assert_eq!(export, "$env:KEY = 'value with spaces'");

        let export = format.format_export("KEY", "value with 'quotes'");
        assert_eq!(export, "$env:KEY = 'value with ''quotes'''");

        let unset = format.format_unset("KEY");
        assert_eq!(unset, "Remove-Item Env:\\KEY");
    }

    #[test]
    fn test_cmd_export_format() {
        let format = WindowsPlatform::get_export_format(Shell::Cmd);
        let export = format.format_export("KEY", "value");
        assert_eq!(export, "set KEY=\"value\"");

        let export = format.format_export("KEY", "value with \"quotes\"");
        assert_eq!(export, "set KEY=\"value with ^\"quotes^\"\"");

        let unset = format.format_unset("KEY");
        assert_eq!(unset, "set KEY");
    }

    #[test]
    fn test_environment_setup() {
        let mut env = HashMap::new();
        env.insert("USERPROFILE".to_string(), "C:\\Users\\test".to_string());

        WindowsPlatform::setup_environment(&mut env);

        // Should add HOME based on USERPROFILE
        assert_eq!(env.get("HOME"), Some(&"C:\\Users\\test".to_string()));
    }
}
