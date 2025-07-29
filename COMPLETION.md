# Shell Completion for cuenv

This demonstrates the shell completion functionality added to cuenv.

## Installation

Generate completion script for your shell:

```bash
# For bash
cuenv completion bash > ~/.local/share/bash-completion/completions/cuenv

# For zsh
cuenv completion zsh > ~/.zsh/completions/_cuenv

# For fish  
cuenv completion fish > ~/.config/fish/completions/cuenv.fish

# For PowerShell
cuenv completion powershell > $PROFILE.CurrentUserAllHosts/../Completions/cuenv.ps1
```

## Features

### Static Completions
- All cuenv commands: `load`, `unload`, `status`, `init`, `allow`, `deny`, `run`, `exec`, `hook`, `export`, `dump`, `prune`, `clear-cache`, `cache`, `remote-cache-server`, `completion`
- All flags: `-h/--help`, `-V/--version`, `-e/--env`, `-c/--capability`, `--audit`

### Dynamic Completions
- **Task names**: When using `cuenv run <TAB>`, completes with available task names from env.cue
- **Environment names**: When using `-e <TAB>` or `--env <TAB>`, completes with environment names from env.cue
- **Allowed hosts**: When using tasks with security restrictions, completes with allowed hosts

## Examples

```bash
# Complete task names
cuenv run <TAB>
# Shows: build, test, deploy, lint, etc.

# Complete environment names  
cuenv run -e <TAB>
# Shows: development, staging, production, etc.

# Complete commands after exec
cuenv exec <TAB>
# Shows standard command completion
```