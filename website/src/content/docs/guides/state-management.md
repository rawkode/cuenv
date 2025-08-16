---
title: State Management
description: Why your shell doesn't lag anymore
---

direnv creates temp files. We don't. That's why cuenv is fast and your shell prompt doesn't stutter.

## State Variables

cuenv maintains its state through several environment variables:

### CUENV_DIR

The directory containing the currently loaded environment file. This is set when an environment is loaded and cleared when unloaded.

```bash
# Check current environment directory
echo $CUENV_DIR
# Output: /home/user/projects/myapp
```

### CUENV_FILE

The full path to the loaded environment file. Useful for debugging and understanding which specific file is active.

```bash
# Check loaded file
echo $CUENV_FILE
# Output: /home/user/projects/myapp/env.cue
```

### CUENV_WATCHES

A colon-separated list of all files being watched for changes. This includes the main env.cue file and any imported files.

```bash
# View all watched files
echo $CUENV_WATCHES | tr ':' '\n'
# Output:
# /home/user/projects/myapp/env.cue
# /home/user/projects/myapp/config/database.cue
# /home/user/projects/myapp/config/secrets.cue
```

### CUENV_DIFF

A base64-encoded representation of the environment changes. This allows cuenv to precisely restore the previous environment when unloading.

## File Watching

cuenv automatically detects changes to your environment files and reloads them:

### How It Works

1. **Initial Load**: When entering a directory, cuenv records the modification time of all CUE files
2. **Change Detection**: On each directory change, cuenv checks if any watched files have been modified
3. **Automatic Reload**: If changes are detected, the environment is automatically reloaded
4. **Dependency Tracking**: Imported CUE files are automatically added to the watch list

### Example

```cue
// env.cue
package cuenv

import "./config/database.cue"
import "./config/api.cue"

// All three files (env.cue, database.cue, api.cue)
// are now watched for changes
```

## State Persistence

The state persists across:

- Directory changes within the same shell
- Subshells and child processes
- Terminal multiplexers (tmux, screen)

The state is cleared when:

- You exit the loaded directory
- You run `cuenv unload`
- The shell session ends

## Performance Optimization

### Efficient File Checks

cuenv uses high-performance file time comparisons instead of content hashing:

```rust
// Pseudocode of the file watching logic
if file.modified_time > last_check_time {
    reload_environment()
}
```

### Minimal Overhead

- No temporary files to create or clean up
- No file system writes during normal operation
- State stored in memory (environment variables)
- Lazy evaluation of file changes

## Debugging State

### View Current State

```bash
# Check if environment is loaded
if [[ -n "$CUENV_DIR" ]]; then
    echo "Environment loaded from: $CUENV_DIR"
    echo "File: $CUENV_FILE"
    echo "Watching $(echo $CUENV_WATCHES | tr ':' ' ' | wc -w) files"
fi
```

### Manual State Inspection

```bash
# Export state for debugging
cuenv dump

# View state changes
env | grep ^CUENV_
```

## Advanced Usage

### Custom File Watching

You can add additional files to the watch list:

```cue
// env.cue
package cuenv

// Watch external configuration
#watch: [
    "./docker-compose.yml",
    "./package.json",
]
```

### State Hooks

React to state changes in your shell:

```bash
# ~/.bashrc
cuenv_on_load() {
    if [[ "$CUENV_DIR" == *"production"* ]]; then
        echo "⚠️  Production environment loaded!"
    fi
}

cuenv_on_unload() {
    echo "Environment unloaded: $CUENV_DIR"
}
```

## Comparison with direnv

| Feature            | cuenv                      | direnv                 |
| ------------------ | -------------------------- | ---------------------- |
| State Storage      | Environment variables      | Temporary files        |
| File Watching      | Built-in with dependencies | Manual configuration   |
| Performance        | No file I/O                | File system operations |
| Cleanup            | Automatic                  | Manual cleanup needed  |
| Multi-file Support | Automatic imports          | `.envrc` only          |

## Best Practices

1. **Keep env.cue files small**: Large files take longer to parse
2. **Use imports**: Split configuration into logical modules
3. **Avoid circular imports**: Can cause reload loops
4. **Test changes**: Use `cuenv status` to verify state

## Troubleshooting

### Environment Not Reloading

Check if files are being watched:

```bash
echo $CUENV_WATCHES
```

Force reload:

```bash
cuenv unload && cuenv load
```

### State Corruption

Clear all state:

```bash
cuenv prune --all
```

### Performance Issues

Enable debug logging:

```bash
export CUENV_DEBUG=1
cd /path/to/project  # Watch the debug output
```

## Security Considerations

### State Visibility

Environment variables are visible to all processes:

```bash
# Other processes can see the state
ps aux | grep CUENV_DIR
```

### Sensitive Data

The CUENV_DIFF variable may contain sensitive data in encoded form. It's automatically cleared when unloading.

## Future Enhancements

Planned improvements to state management:

- **Incremental reloading**: Only reload changed portions
- **State compression**: Reduce memory usage for large environments
- **Cross-shell state**: Share state between shell sessions
- **State persistence**: Optional file-based state backup
