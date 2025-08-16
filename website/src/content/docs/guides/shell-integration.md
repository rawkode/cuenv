---
title: Shell Integration
description: How to integrate cuenv with your shell for automatic environment loading
---

cuenv integrates seamlessly with popular shells to automatically load and unload environment variables as you navigate directories.

## Supported Shells

- **Bash** (Linux, macOS, Windows via WSL/Git Bash)
- **Zsh** (macOS default, Linux)
- **Fish** (Cross-platform)

## Installation by Shell

### Bash

Add to your `~/.bashrc` or `~/.bash_profile`:

```bash title="~/.bashrc"
eval "$(cuenv shell init bash)"
```

For system-wide installation, add to `/etc/bash.bashrc`:

```bash title="/etc/bash.bashrc"
# System-wide cuenv for all users
if command -v cuenv >/dev/null 2>&1; then
    eval "$(cuenv shell init bash)"
fi
```

### Zsh

Add to your `~/.zshrc`:

```zsh title="~/.zshrc"
eval "$(cuenv shell init zsh)"
```

For Oh My Zsh users, you can create a custom plugin:

```bash
# Create custom plugin directory
mkdir -p ~/.oh-my-zsh/custom/plugins/cuenv

# Create plugin file
cat > ~/.oh-my-zsh/custom/plugins/cuenv/cuenv.plugin.zsh << 'EOF'
if command -v cuenv >/dev/null 2>&1; then
    eval "$(cuenv shell init zsh)"
fi
EOF

# Add to your .zshrc plugins list
plugins=(... cuenv)
```

### Fish

Add to your `~/.config/fish/config.fish`:

```fish title="~/.config/fish/config.fish"
if command -v cuenv >/dev/null 2>&1
    cuenv shell init fish | source
end
```

Or create a Fish function:

```bash
# Create the function file
mkdir -p ~/.config/fish/functions
cat > ~/.config/fish/functions/cuenv_init.fish << 'EOF'
function cuenv_init --description 'Initialize cuenv'
    cuenv shell init fish | source
end
end
EOF

# Add to config.fish
echo "cuenv_init" >> ~/.config/fish/config.fish
```

## How Shell Integration Works

### Hook Mechanism

cuenv uses shell hooks to detect directory changes:

1. **Bash**: Uses `PROMPT_COMMAND`
1. **Zsh**: Uses `precmd` hook
1. **Fish**: Uses `fish_prompt` event

### The Hook Flow

```mermaid
graph TD
    A[Change Directory] --> B[Shell Hook Triggered]
    B --> C[cuenv Checks for env.cue]
    C --> D{env.cue found?}
    D -->|Yes| E[Load Environment]
    D -->|No| F[Unload Previous Environment]
    E --> G[Set Variables]
    F --> H[Restore Variables]
```

## Customization

### Custom Prompt Integration

Show when cuenv environment is active:

#### Bash

```bash
# Add to .bashrc after cuenv init
cuenv_prompt() {
    if [[ -n "$CUENV_DIR" ]]; then
        echo " (cuenv)"
    fi
}

PS1="\u@\h:\w\$(cuenv_prompt)\$ "
```

#### Zsh

```zsh
# Add to .zshrc
setopt PROMPT_SUBST
PROMPT='%n@%m:%~$(cuenv_prompt) %# '

cuenv_prompt() {
    if [[ -n "$CUENV_DIR" ]]; then
        echo " %F{green}(cuenv)%f"
    fi
}
```

#### Fish

```fish
# Add to config.fish
function fish_prompt
    if set -q CUENV_DIR
        echo -n (set_color green)"(cuenv) "(set_color normal)
    end
    # Your existing prompt
    echo -n (whoami)'@'(hostname)':'(pwd)'> '
end
```

### Starship Prompt

If using [Starship](https://starship.rs/), add a custom module:

```toml title="~/.config/starship.toml"
[custom.cuenv]
command = 'echo "📦"'
when = '[[ -n "$CUENV_DIR" ]]'
format = '[$output]($style) '
style = "bold green"
```

## Performance Optimization

### Lazy Loading

For faster shell startup, use lazy loading:

#### Bash

```bash title="~/.bashrc"
# Lazy load cuenv
cuenv() {
    unset -f cuenv
    eval "$(command cuenv shell init bash)"
    cuenv "$@"
}
```

#### Zsh

```zsh title="~/.zshrc"
# Lazy load cuenv
cuenv() {
    unfunction cuenv
    eval "$(command cuenv shell init zsh)"
    cuenv "$@"
}
```

### State Management

cuenv uses environment variables to track state instead of files, providing better performance and reliability:

- **CUENV_DIR** - Current environment directory
- **CUENV_FILE** - Loaded environment file path
- **CUENV_WATCHES** - Files being watched for changes
- **CUENV_DIFF** - Environment modifications for restoration

The state is automatically managed by the shell hooks and persists across shell sessions.

### File Watching

cuenv automatically watches imported files and reloads the environment when they change:

- Tracks modification times of all imported CUE files
- Automatically reloads when watched files are modified
- Minimal performance overhead using efficient file time checks

## Shell-Specific Features

### Shell Completion

cuenv provides comprehensive tab completion for all commands, flags, and dynamic content like task names and environment names.

#### Installation

Generate and install completion scripts for your shell:

##### Bash

```bash title="~/.bashrc"
# Enable cuenv completion
if command -v cuenv >/dev/null 2>&1; then
    eval "$(cuenv completion bash)"
fi
```

For system-wide completion:

```bash
# Save to system completion directory
sudo cuenv completion bash > /etc/bash_completion.d/cuenv
```

##### Zsh

```zsh title="~/.zshrc"
# Enable cuenv completion
if command -v cuenv >/dev/null 2>&1; then
    eval "$(cuenv completion zsh)"
fi
```

For manual installation:

```bash
# Create completion directory if it doesn't exist
mkdir -p ~/.zsh/completions

# Generate completion script
cuenv completion zsh > ~/.zsh/completions/_cuenv

# Add to .zshrc
fpath=(~/.zsh/completions $fpath)
autoload -U compinit && compinit
```

##### Fish

```fish title="~/.config/fish/config.fish"
# Enable cuenv completion
if command -v cuenv >/dev/null 2>&1
    cuenv completion fish | source
end
```

For persistent installation:

```bash
# Create completions directory
mkdir -p ~/.config/fish/completions

# Generate completion script
cuenv completion fish > ~/.config/fish/completions/cuenv.fish
```

##### PowerShell

```powershell title="Microsoft.PowerShell_profile.ps1"
# Enable cuenv completion
if (Get-Command cuenv -ErrorAction SilentlyContinue) {
    cuenv completion powershell | Out-String | Invoke-Expression
}
```

##### Elvish

```elvish title="~/.config/elvish/rc.elv"
# Enable cuenv completion (if elvish support is available)
if (has-external cuenv) {
    eval (cuenv completion elvish | slurp)
}
```

#### Completion Features

The completion system provides intelligent suggestions for:

##### Static Completions

- **Commands**: `task`, `env`, `shell`, `discover`, `cache`, `exec`, `init`, `completion`, `mcp`
- **Global flags**: `-h/--help`, `-V/--version`, `-e/--env`, `-c/--capability`, `--audit`
- **Command-specific flags**: Context-sensitive options for each command

##### Dynamic Completions

- **Task names**: When using `cuenv task <TAB>`, completes with available task names from `env.cue`
- **Environment names**: When using `-e <TAB>` or `--env <TAB>`, completes with defined environments
- **Capability names**: When using `-c <TAB>` or `--capability <TAB>`, completes with available capabilities
- **Allowed hosts**: Context-aware host completion for security-restricted tasks

#### Usage Examples

```bash
# Complete available tasks
cuenv task <TAB>
# Shows: build, test, deploy, lint, format, etc.

# Complete environment names
cuenv exec -e <TAB>
# Shows: development, staging, production, etc.

# Complete capability names
cuenv exec -c <TAB>
# Shows: network, filesystem, secrets, etc.
```

cuenv <TAB>

# Shows all available commands with descriptions

````

#### Advanced Completion

The completion system understands CUE file structure and provides:

- **Environment-specific tasks**: Only shows tasks available in the selected environment
- **Conditional capabilities**: Suggests capabilities based on current task context
- **File path completion**: Smart completion for CUE files and configuration paths
- **Host completion**: Security-aware hostname completion for allowed hosts

#### Troubleshooting Completion

If completion isn't working:

1. **Verify installation**:

   ```bash
   # Check if cuenv is in PATH
   which cuenv

   # Test completion generation
   cuenv completion bash
````

2. **Reload shell configuration**:

   ```bash
   # Bash
   source ~/.bashrc

   # Zsh
   source ~/.zshrc

   # Fish
   source ~/.config/fish/config.fish
   ```

3. **Check for conflicts**:
   - Ensure no other tools are overriding cuenv completion
   - Verify completion functions are loaded correctly
   - Check for shell-specific completion settings

## Troubleshooting

### Environment Not Loading

1. **Check initialization:**

   ```bash
   # Verify cuenv is initialized
   echo $PROMPT_COMMAND | grep cuenv  # Bash
   echo $precmd_functions | grep cuenv  # Zsh
   ```

1. **Manual test:**

   ```bash
   # Test hook manually
   cuenv shell hook bash  # or zsh, fish
   ```

1. **Debug mode:**

   ```bash
   export CUENV_DEBUG=1
   cd /path/to/project
   ```

### Variables Not Unloading

If variables persist after leaving a directory:

```bash
# Force unload
cuenv shell unload

# Check current status
cuenv env status
```

### Conflicts with Other Tools

If using direnv or similar tools:

```bash
# Load cuenv after other tools
eval "$(direnv hook bash)"
eval "$(cuenv shell init bash)"  # Load after direnv
```

## Advanced Configuration

### Custom File Names

By default, cuenv looks for `env.cue`. To use custom filenames:

```bash
# Set in your shell config
export CUENV_FILENAME="environment.cue"
```

### Disable Auto-loading

To disable automatic loading but keep commands available:

```bash
# Add to shell config
export CUENV_DISABLE_AUTO=1

# Manually load when needed
cuenv shell load
```

### Custom Load/Unload Hooks

Run custom commands on environment changes:

```bash title="~/.bashrc"
# Bash example
cuenv_post_load() {
    echo "Loaded environment from: $CUENV_DIR"
    # Custom actions here
}

cuenv_post_unload() {
    echo "Unloaded environment"
    # Cleanup actions here
}

# Export functions for cuenv to use
export -f cuenv_post_load cuenv_post_unload
```

## Integration with Tools

### tmux

Ensure cuenv works in new tmux panes/windows:

```bash title="~/.tmux.conf"
set-option -g default-command "${SHELL}"
set-option -g update-environment "CUENV_DIR CUENV_FILE"
```

### VS Code

For integrated terminal support:

```json title="settings.json"
{
	"terminal.integrated.env.linux": {
		"CUENV_INIT": "1"
	},
	"terminal.integrated.profiles.linux": {
		"bash": {
			"path": "bash",
			"args": ["-l"]
		}
	}
}
```

### Docker

Use cuenv with Docker:

```dockerfile title="Dockerfile"
FROM ubuntu:latest

# Install cuenv
RUN cargo install cuenv

# Initialize in shell
RUN echo 'eval "$(cuenv init bash)"' >> /etc/bash.bashrc

# Copy env.cue
COPY env.cue /app/env.cue
WORKDIR /app

# Commands will have access to cuenv environment
CMD ["bash", "-l", "-c", "your-command"]
```

## Platform-Specific Notes

### macOS

On macOS with Homebrew:

```bash
# If using Homebrew's bash
echo 'eval "$(cuenv init bash)"' >> ~/.bash_profile

# For system bash
echo 'eval "$(cuenv init bash)"' >> ~/.bashrc
```

### Windows

#### Git Bash

```bash title="~/.bashrc"
eval "$(cuenv init bash)"
```

#### WSL (Windows Subsystem for Linux)

Works the same as Linux:

```bash title="~/.bashrc"
eval "$(cuenv init bash)"
```

```zsh title="~/.zshrc"
eval "$(cuenv init zsh)"
```

#### PowerShell

PowerShell support is planned for future releases. Currently, use Git Bash or WSL.

### Linux

Most Linux distributions work out of the box. For non-standard shells or configurations:

```bash
# Test the hook manually
cuenv hook bash > /tmp/cuenv-hook.sh
cat /tmp/cuenv-hook.sh  # Inspect the hook
```

## Tips and Tricks

### Quick Environment Check

Add an alias to quickly check current environment:

```bash title="~/.bashrc or ~/.zshrc"
alias ce='cuenv status'
```

```fish title="~/.config/fish/config.fish"
alias ce 'cuenv status'
```

### Project Templates

Create a template for new projects:

```bash title="~/.bashrc or ~/.zshrc"
# Create template function
new_project() {
    mkdir -p "$1"
    cd "$1"
    cat > env.cue << 'EOF'
package cuenv

PROJECT_NAME: "$1"
ENVIRONMENT: "development"
DEBUG: true
EOF
    echo "Created new project: $1"
}
```

### Environment Switching

Quickly switch between environments:

```bash title="~/.bashrc or ~/.zshrc"
# Environment switching function
cue_env() {
    local env=${1:-development}
    CUENV_ENV=$env bash -l
}

# Usage
cue_env production  # Opens new shell with production env
```
