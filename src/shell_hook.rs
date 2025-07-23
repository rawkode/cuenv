use crate::errors::{Error, Result};
use std::path::PathBuf;

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
                format!("unsupported shell: {}", shell),
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

    pub fn generate_hook_output(shell: &str, current_dir: &PathBuf) -> Result<String> {
        let cuenv_file = current_dir.join(".cuenv_current");
        let env_cue_exists = current_dir.join("env.cue").exists();

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
                format!("unsupported shell: {}", shell),
            )),
        }
    }
}
