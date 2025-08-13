# Starship Module for cuenv Hook Progress

This module provides real-time progress tracking for cuenv preload hooks directly in your terminal prompt using [Starship](https://starship.rs/).

## Features

- **Non-intrusive**: Only visible during hook execution
- **Real-time updates**: Shows progress as hooks run
- **Auto-hide**: Disappears after hooks complete (5-second grace period)
- **Multiple formats**: Aggregate or verbose output
- **Customizable**: Control colors, position, and styling

## Installation

### Prerequisites

1. [Starship prompt](https://starship.rs/) installed and configured
2. cuenv with hook support enabled
3. Environment with preload hooks configured

### Setup

1. Choose a configuration from `starship.toml` in this directory
2. Add it to your Starship configuration file (usually `~/.config/starship.toml`)
3. Reload your shell or restart your terminal

### Quick Start

Add this basic configuration to your `starship.toml`:

```toml
[custom.cuenv_hooks]
command = "cuenv env status --hooks --format=starship"
when = """ test -n "$CUENV_DIR" """
format = "$output"
disabled = false
```

## Output States

The module displays different indicators based on hook status:

| Indicator            | Description                      | Duration      |
| -------------------- | -------------------------------- | ------------- |
| `⏳ 2/3 hooks (15s)` | Hooks are running                | While running |
| `✅ Hooks ready`     | All hooks completed successfully | 5 seconds     |
| `⚠️ 1 hook failed`   | One or more hooks failed         | 5 seconds     |
| (empty)              | No hooks running                 | After timeout |

## Configuration Options

### Basic Mode

Shows aggregated progress (e.g., "2/3 hooks"):

```toml
[custom.cuenv_hooks]
command = "cuenv env status --hooks --format=starship"
when = """ test -n "$CUENV_DIR" """
format = "$output"
```

### Verbose Mode

Shows individual hook details:

```toml
[custom.cuenv_hooks]
command = "cuenv env status --hooks --format=starship --verbose"
when = """ test -n "$CUENV_DIR" """
format = "$output"
```

Output example: `🔄 nix develop (12s)`

### Right Prompt

Place the indicator on the right side:

```toml
format = "... $custom.cuenv_hooks"
right_format = "$custom.cuenv_hooks_right"

[custom.cuenv_hooks_right]
command = "cuenv env status --hooks --format=starship"
when = """ test -n "$CUENV_DIR" """
format = "[$output]($style)"
style = "dimmed"
```

### Custom Colors

Apply custom styling:

```toml
[custom.cuenv_hooks]
command = "cuenv env status --hooks --format=starship"
when = """ test -n "$CUENV_DIR" """
format = "[$output]($style) "
style = "bold yellow"
```

### Color by Duration

You can create a script to color-code based on duration:

```bash
#!/bin/bash
# ~/.config/starship/cuenv-hooks-colored.sh
output=$(cuenv env status --hooks --format=starship)
if [[ -z "$output" ]]; then
    exit 0
fi

# Extract duration if present
if [[ "$output" =~ \(([0-9]+)s\) ]]; then
    duration=${BASH_REMATCH[1]}
    if [ $duration -lt 10 ]; then
        echo -e "\033[32m$output\033[0m"  # Green
    elif [ $duration -lt 30 ]; then
        echo -e "\033[33m$output\033[0m"  # Yellow
    else
        echo -e "\033[31m$output\033[0m"  # Red
    fi
else
    echo "$output"
fi
```

## Testing

### Manual Testing

Test the status command directly:

```bash
# Basic output
cuenv env status --hooks --format=starship

# Verbose output
cuenv env status --hooks --format=starship --verbose

# Human-readable format
cuenv env status --hooks

# JSON format (for debugging)
cuenv env status --hooks --format=json
```

### Simulating Hooks

Create a test directory with preload hooks:

```cue
// .cuenv.cue
package env

import "time"

hooks: {
    onEnter: [
        {
            command: "sleep"
            args: ["5"]
            preload: true
        },
        {
            command: "echo"
            args: ["Loading environment..."]
            preload: true
        },
    ]
}
```

Then `cd` into the directory to trigger the hooks and see the status in your prompt.

## Troubleshooting

### Module not showing

1. Verify `CUENV_DIR` is set: `echo $CUENV_DIR`
2. Check if hooks are configured: `cuenv env status --hooks`
3. Ensure Starship custom modules are enabled
4. Test the command manually (see Testing section)

### Performance issues

- The status command reads from a cached JSON file, so it should be fast
- If slow, check: `time cuenv env status --hooks --format=starship`
- Consider increasing Starship's command timeout if needed

### Incorrect status

- Status file might be stale: Check `/tmp/cuenv-$USER/hooks-status.json`
- Hooks might have been killed: The status clears automatically on directory change
- Check logs: `RUST_LOG=debug cuenv env status --hooks`

## Integration with Other Tools

### tmux

Add to your tmux status line:

```tmux
set -g status-right '#(cuenv env status --hooks --format=starship)'
```

### Vim/Neovim

Use in your statusline:

```vim
set statusline+=%{system('cuenv env status --hooks --format=starship')}
```

### VS Code

The cuenv VS Code extension can also display hook progress in the status bar.

## Advanced Customization

### Custom Icons

Modify the source code or wrap the command to use different icons:

```bash
#!/bin/bash
output=$(cuenv env status --hooks --format=starship)
echo "${output//⏳/🚀}"  # Replace hourglass with rocket
```

### Nerd Font Icons

If you have Nerd Fonts installed, you can use tool-specific icons:

- Nix: or
- Node:
- Docker:
- Python:
- Rust:

## Contributing

To improve the Starship module:

1. Fork the cuenv repository
2. Modify `crates/cli/src/commands/env/status.rs`
3. Test your changes
4. Submit a pull request

## See Also

- [cuenv Documentation](https://github.com/rawkode/cuenv)
- [Starship Documentation](https://starship.rs/config/)
- [Hook Configuration Guide](../hooks-preload/README.md)
