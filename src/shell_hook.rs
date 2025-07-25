use crate::errors::{Error, Result};

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
    local current_dir="$PWD"
    local cuenv_file="$PWD/.cuenv_current"
    local cuenv_allowed="$HOME/.config/cuenv/allowed"
    
    # Check if current directory has env.cue
    if [[ -f "$current_dir/env.cue" ]]; then
        # Check if directory is allowed
        if [[ -f "$cuenv_allowed" ]] && grep -q "^$current_dir$" "$cuenv_allowed"; then
            # Load environment if not already loaded for this directory
            if [[ ! -f "$cuenv_file" ]] || [[ "$(cat "$cuenv_file" 2>/dev/null)" != "$current_dir" ]]; then
                eval "$(cuenv load)"
                echo "$current_dir" > "$cuenv_file"
            fi
        else
            echo "cuenv: Directory not allowed. Run 'cuenv allow' to allow this directory." >&2
        fi
    else
        # If we have a .cuenv_current file, we need to unload
        if [[ -f "$cuenv_file" ]]; then
            local previous_dir="$(cat "$cuenv_file")"
            # Only unload if we're leaving a directory that had env.cue
            if [[ "$previous_dir" != "$current_dir" ]] && [[ -f "$previous_dir/env.cue" ]]; then
                eval "$(cuenv unload)"
                rm -f "$cuenv_file"
            fi
        fi
    fi
    
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
    local current_dir="$PWD"
    local cuenv_file="$PWD/.cuenv_current"
    local cuenv_allowed="$HOME/.config/cuenv/allowed"
    
    # Check if current directory has env.cue
    if [[ -f "$current_dir/env.cue" ]]; then
        # Check if directory is allowed
        if [[ -f "$cuenv_allowed" ]] && grep -q "^$current_dir$" "$cuenv_allowed"; then
            # Load environment if not already loaded for this directory
            if [[ ! -f "$cuenv_file" ]] || [[ "$(cat "$cuenv_file" 2>/dev/null)" != "$current_dir" ]]; then
                eval "$(cuenv load)"
                echo "$current_dir" > "$cuenv_file"
            fi
        else
            echo "cuenv: Directory not allowed. Run 'cuenv allow' to allow this directory." >&2
        fi
    else
        # If we have a .cuenv_current file, we need to unload
        if [[ -f "$cuenv_file" ]]; then
            local previous_dir="$(cat "$cuenv_file")"
            # Only unload if we're leaving a directory that had env.cue
            if [[ "$previous_dir" != "$current_dir" ]] && [[ -f "$previous_dir/env.cue" ]]; then
                eval "$(cuenv unload)"
                rm -f "$cuenv_file"
            fi
        fi
    fi
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
    set current_dir "$PWD"
    set cuenv_file "$PWD/.cuenv_current"
    set cuenv_allowed "$HOME/.config/cuenv/allowed"
    
    # Check if current directory has env.cue
    if test -f "$current_dir/env.cue"
        # Check if directory is allowed
        if test -f "$cuenv_allowed"; and grep -q "^$current_dir\$" "$cuenv_allowed"
            # Load environment if not already loaded for this directory
            if not test -f "$cuenv_file"
                or test (cat "$cuenv_file" 2>/dev/null) != "$current_dir"
                cuenv load | source
                echo "$current_dir" > "$cuenv_file"
            end
        else
            echo "cuenv: Directory not allowed. Run 'cuenv allow' to allow this directory." >&2
        end
    else
        # If we have a .cuenv_current file, we need to unload
        if test -f "$cuenv_file"
            set previous_dir (cat "$cuenv_file")
            # Only unload if we're leaving a directory that had env.cue
            if test "$previous_dir" != "$current_dir"; and test -f "$previous_dir/env.cue"
                cuenv unload | source
                rm -f "$cuenv_file"
            end
        end
    end
end

_cuenv_hook
"#
        .to_string()
    }

    fn powershell_hook() -> String {
        r#"
function Invoke-CuenvHook {
    $currentDir = $PWD.Path
    $cuenvFile = Join-Path $currentDir ".cuenv_current"
    $cuenvAllowed = Join-Path $env:USERPROFILE ".config\cuenv\allowed"
    
    # Check if current directory has env.cue
    if (Test-Path (Join-Path $currentDir "env.cue")) {
        # Check if directory is allowed
        if ((Test-Path $cuenvAllowed) -and (Get-Content $cuenvAllowed | Select-String -Pattern "^$([regex]::Escape($currentDir))$")) {
            # Load environment if not already loaded for this directory
            if (-not (Test-Path $cuenvFile) -or ((Get-Content $cuenvFile -ErrorAction SilentlyContinue) -ne $currentDir)) {
                $output = cuenv load
                if ($output) {
                    Invoke-Expression $output
                }
                Set-Content -Path $cuenvFile -Value $currentDir
            }
        } else {
            Write-Error "cuenv: Directory not allowed. Run 'cuenv allow' to allow this directory."
        }
    } else {
        # If we have a .cuenv_current file, we need to unload
        if (Test-Path $cuenvFile) {
            $previousDir = Get-Content $cuenvFile
            # Only unload if we're leaving a directory that had env.cue
            if ($previousDir -ne $currentDir -and (Test-Path (Join-Path $previousDir "env.cue"))) {
                $output = cuenv unload
                if ($output) {
                    Invoke-Expression $output
                }
                Remove-Item $cuenvFile -Force
            }
        }
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

set current_dir=%CD%
set cuenv_file=%CD%\.cuenv_current
set cuenv_allowed=%USERPROFILE%\.config\cuenv\allowed

REM Check if current directory has env.cue
if exist "%current_dir%\env.cue" (
    REM Check if directory is allowed
    findstr /b /e /c:"%current_dir%" "%cuenv_allowed%" >nul 2>&1
    if %errorlevel% equ 0 (
        REM Load environment if not already loaded
        if not exist "%cuenv_file%" (
            for /f "tokens=*" %%i in ('cuenv load') do %%i
            echo %current_dir% > "%cuenv_file%"
        ) else (
            set /p prev_dir=<"%cuenv_file%"
            if not "%prev_dir%"=="%current_dir%" (
                for /f "tokens=*" %%i in ('cuenv load') do %%i
                echo %current_dir% > "%cuenv_file%"
            )
        )
    ) else (
        echo cuenv: Directory not allowed. Run 'cuenv allow' to allow this directory. >&2
    )
) else (
    REM Unload if leaving a directory with env.cue
    if exist "%cuenv_file%" (
        set /p prev_dir=<"%cuenv_file%"
        if exist "%prev_dir%\env.cue" (
            for /f "tokens=*" %%i in ('cuenv unload') do %%i
            del "%cuenv_file%"
        )
    )
)
"#
        .to_string()
    }
}
