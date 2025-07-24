use crate::errors::{Error, Result};
use std::path::{Path, PathBuf};

use crate::cue_parser::{CueParser, HookConfig, HookType, ParseOptions};
use crate::platform::{escape_cmd_value, escape_shell_value};

pub struct ShellHook;

impl ShellHook {
    pub fn generate_hook(shell: &str) -> Result<String> {
        match shell {
            "bash" => Ok(Self::bash_hook()),
            "zsh" => Ok(Self::zsh_hook()),
            "fish" => Ok(Self::fish_hook()),
            "powershell" => Ok(Self::powershell_hook()),
            "cmd" => Ok(Self::cmd_hook()),
            _ => Err(Error::unsupported(
                "shell",
                format!("unsupported shell: {shell}"),
            )),
        }
    }

    fn bash_hook() -> String {
        r#"
_cuenv_hook() {
    local previous_exit_status=$?
    eval "$(cuenv hook bash)"
    return $previous_exit_status
}

if [[ ";${PROMPT_COMMAND:-};" != *";_cuenv_hook;"* ]]; then
    PROMPT_COMMAND="_cuenv_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
"#
        .to_string()
    }

    fn zsh_hook() -> String {
        r#"
_cuenv_hook() {
    eval "$(cuenv hook zsh)"
}

typeset -ag precmd_functions
if [[ -z ${precmd_functions[(r)_cuenv_hook]} ]]; then
    precmd_functions+=(_cuenv_hook)
fi
"#
        .to_string()
    }

    fn fish_hook() -> String {
        r#"
function _cuenv_hook --on-variable PWD
    cuenv hook fish | source
end

_cuenv_hook
"#
        .to_string()
    }

    fn powershell_hook() -> String {
        r#"
function Invoke-CuenvHook {
    $output = cuenv hook powershell
    if ($output) {
        Invoke-Expression $output
    }
}

# Set up location change detection
$ExecutionContext.SessionState.InvokeCommand.LocationChangedAction = {
    Invoke-CuenvHook
}

# Initial hook
Invoke-CuenvHook
"#
        .to_string()
    }

    fn cmd_hook() -> String {
        // CMD doesn't support automatic hooks, so we provide a manual function
        r#"
@echo off
REM Add this to your CMD startup script or call manually:
REM cuenv_hook.cmd

for /f "tokens=*" %%i in ('cuenv hook cmd') do (
    %%i
)
"#
        .to_string()
    }

    pub fn generate_hook_output(shell: &str, current_dir: &Path) -> Result<String> {
        let cuenv_file = current_dir.join(".cuenv_current");
        let env_cue_exists = current_dir.join("env.cue").exists();

        // Check if we have a previous environment stored
        let previous_dir = if cuenv_file.exists() {
            match std::fs::read_to_string(&cuenv_file) {
                Ok(content) => {
                    let dir = PathBuf::from(content.trim());
                    if dir != current_dir && dir.join("env.cue").exists() {
                        Some(dir)
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        };

        let mut hook_commands = String::new();

        // If we're leaving a previous environment
        if let Some(prev_dir) = previous_dir {
            // Load hooks from the previous directory
            let options = ParseOptions::default();
            match CueParser::eval_package_with_options(&prev_dir, "env", &options) {
                Ok(parse_result) => {
                    // Filter for onExit hooks
                    let exit_hooks: Vec<_> = parse_result
                        .hooks
                        .iter()
                        .filter(|(_, config)| config.hook_type == HookType::OnExit)
                        .collect();

                    if !exit_hooks.is_empty() {
                        log::debug!(
                            "Generating onExit hook commands for: {}",
                            prev_dir.display()
                        );
                        for (name, config) in exit_hooks {
                            // Generate shell command to execute the hook
                            hook_commands
                                .push_str(&Self::generate_hook_command(shell, name, config)?);
                            hook_commands.push('\n');
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Failed to load hooks from previous dir {}: {}",
                        prev_dir.display(),
                        e
                    );
                }
            }
        }

        // Generate the shell-specific hook output
        let env_output =
            Self::generate_shell_specific_output(shell, current_dir, &cuenv_file, env_cue_exists)?;

        // Combine hook commands with environment management
        if hook_commands.is_empty() {
            Ok(env_output)
        } else {
            Ok(format!("{hook_commands}{env_output}"))
        }
    }

    fn generate_hook_command(shell: &str, name: &str, config: &HookConfig) -> Result<String> {
        match shell {
            "bash" | "zsh" => {
                let args = config
                    .args
                    .iter()
                    .map(|arg| escape_shell_value(arg))
                    .collect::<Vec<_>>()
                    .join(" ");

                if let Some(url) = &config.url {
                    Ok(format!("# onExit hook: {name}\ncurl -s '{url}' | sh"))
                } else {
                    Ok(format!("# onExit hook: {name}\n{} {args}", config.command))
                }
            }
            "fish" => {
                let args = config
                    .args
                    .iter()
                    .map(|arg| escape_shell_value(arg))
                    .collect::<Vec<_>>()
                    .join(" ");

                if let Some(url) = &config.url {
                    Ok(format!("# onExit hook: {name}\ncurl -s '{url}' | sh"))
                } else {
                    Ok(format!("# onExit hook: {name}\n{} {args}", config.command))
                }
            }
            "powershell" => {
                let args = config
                    .args
                    .iter()
                    .map(|arg| format!("'{}'", arg.replace("'", "''")))
                    .collect::<Vec<_>>()
                    .join(" ");

                if let Some(url) = &config.url {
                    Ok(format!(
                        "# onExit hook: {name}\nInvoke-WebRequest -Uri '{url}' -UseBasicParsing | Select-Object -ExpandProperty Content | Invoke-Expression"
                    ))
                } else {
                    Ok(format!(
                        "# onExit hook: {name}\n& '{}' {args}",
                        config.command
                    ))
                }
            }
            "cmd" => {
                let args = config
                    .args
                    .iter()
                    .map(|arg| escape_cmd_value(arg))
                    .collect::<Vec<_>>()
                    .join(" ");

                if let Some(url) = &config.url {
                    Ok(format!("REM onExit hook: {name}\ncurl -s \"{url}\" | cmd"))
                } else {
                    Ok(format!(
                        "REM onExit hook: {name}\n{} {args}",
                        config.command
                    ))
                }
            }
            _ => Err(Error::unsupported(
                "shell",
                format!("unsupported shell: {shell}"),
            )),
        }
    }

    fn generate_shell_specific_output(
        shell: &str,
        current_dir: &Path,
        cuenv_file: &Path,
        env_cue_exists: bool,
    ) -> Result<String> {
        match shell {
            "bash" | "zsh" => {
                if env_cue_exists {
                    Ok(format!(
                        r#"
if [[ ! -f "{}" ]] || [[ "$(cat "{}" 2>/dev/null)" != "{}" ]]; then
    cuenv load
    echo "{}" > "{}"
fi
"#,
                        cuenv_file.display(),
                        cuenv_file.display(),
                        current_dir.display(),
                        current_dir.display(),
                        cuenv_file.display()
                    ))
                } else if cuenv_file.exists() {
                    Ok(format!(
                        r#"
if [[ -f "{}" ]]; then
    cuenv unload
    rm -f "{}"
fi
"#,
                        cuenv_file.display(),
                        cuenv_file.display()
                    ))
                } else {
                    Ok(String::new())
                }
            }
            "fish" => {
                if env_cue_exists {
                    Ok(format!(
                        r#"
if not test -f "{}"
    or test (cat "{}" 2>/dev/null) != "{}"
    cuenv load | source
    echo "{}" > "{}"
end
"#,
                        cuenv_file.display(),
                        cuenv_file.display(),
                        current_dir.display(),
                        current_dir.display(),
                        cuenv_file.display()
                    ))
                } else if cuenv_file.exists() {
                    Ok(format!(
                        r#"
if test -f "{}"
    cuenv unload | source
    rm -f "{}"
end
"#,
                        cuenv_file.display(),
                        cuenv_file.display()
                    ))
                } else {
                    Ok(String::new())
                }
            }
            "powershell" => {
                if env_cue_exists {
                    Ok(format!(
                        r#"
if (-not (Test-Path "{}") -or ((Get-Content "{}" 2>$null) -ne "{}")) {{
    $output = cuenv load
    if ($output) {{
        Invoke-Expression $output
    }}
    Set-Content -Path "{}" -Value "{}"
}}
"#,
                        cuenv_file.display(),
                        cuenv_file.display(),
                        current_dir.display(),
                        cuenv_file.display(),
                        current_dir.display()
                    ))
                } else if cuenv_file.exists() {
                    Ok(format!(
                        r#"
if (Test-Path "{}") {{
    $output = cuenv unload
    if ($output) {{
        Invoke-Expression $output
    }}
    Remove-Item "{}"
}}
"#,
                        cuenv_file.display(),
                        cuenv_file.display()
                    ))
                } else {
                    Ok(String::new())
                }
            }
            "cmd" => {
                if env_cue_exists {
                    Ok(format!(
                        r#"
@echo off
if not exist "{}" goto :load
set /p current_dir=<"{}"
if not "%current_dir%"=="{}" goto :load
goto :end

:load
cuenv load
echo {}> "{}"

:end
"#,
                        cuenv_file.display(),
                        cuenv_file.display(),
                        current_dir.display(),
                        current_dir.display(),
                        cuenv_file.display()
                    ))
                } else if cuenv_file.exists() {
                    Ok(format!(
                        r#"
@echo off
if exist "{}" (
    cuenv unload
    del "{}"
)
"#,
                        cuenv_file.display(),
                        cuenv_file.display()
                    ))
                } else {
                    Ok(String::new())
                }
            }
            _ => Err(Error::unsupported(
                "shell",
                format!("unsupported shell: {shell}"),
            )),
        }
    }
}
