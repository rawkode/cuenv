# cuenv VSCode Extension

A powerful VSCode extension that provides seamless integration with [cuenv](https://github.com/rawkode/cuenv), bringing direnv-style environment management and task execution directly to your editor.

## 🚀 Features

### Environment Management
- **Auto Environment Loading**: Automatically detects and loads `env.cue` files when opening workspace folders
- **Smart Status Bar**: Real-time environment status with quick actions (✓ Loaded, ↺ Pending Reload, ⚠ Error)
- **Environment Panel**: Browse all environment variables with smart masking for sensitive data
- **Change Detection**: Automatic detection of `env.cue` file changes with reload prompts

### Task Integration
- **Tasks Panel**: View all available tasks with descriptions and dependencies
- **CodeLens Integration**: Inline "Run Task" buttons directly in `env.cue` files
- **Terminal Integration**: Execute tasks in shared or new terminals
- **Task Dependencies**: Visual representation of task dependencies and execution order

### Multi-Root Workspace Support
- **Independent Management**: Each workspace folder has its own environment and task state
- **Context Awareness**: Status and panels adapt based on the active editor's workspace folder
- **Scalable**: Handles monorepo setups with multiple `env.cue` files

### Security & Privacy
- **Smart Masking**: Configurable regex patterns automatically hide sensitive variables
- **Safe Logging**: Sensitive values are masked in output logs
- **Copy Protection**: Copy operations always use unmasked values

## 📋 Requirements

- **cuenv**: Requires cuenv CLI to be installed and available in PATH
- **VSCode**: Version 1.74.0 or higher

### Installing cuenv

Visit the [cuenv releases page](https://github.com/rawkode/cuenv/releases) to download the latest binary for your platform.

## ⚙️ Configuration

### Extension Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `cuenv.executablePath` | string | `"cuenv"` | Path to the cuenv executable |
| `cuenv.autoLoad.enabled` | boolean | `true` | Automatically load environment on workspace open |
| `cuenv.env.maskPatterns` | string[] | `["(?i)(secret\\|token\\|password\\|key\\|api_key)"]` | Regex patterns for masking sensitive variables |
| `cuenv.tasks.terminal.strategy` | enum | `"shared"` | Terminal strategy: `"shared"` or `"new"` |
| `cuenv.watch.debounceMs` | number | `300` | File watch debounce time in milliseconds |

### Example Settings

```json
{
  "cuenv.executablePath": "/usr/local/bin/cuenv",
  "cuenv.autoLoad.enabled": true,
  "cuenv.env.maskPatterns": [
    "(?i)(secret|token|password|key)",
    "(?i).*_SECRET$",
    "(?i).*_TOKEN$"
  ],
  "cuenv.tasks.terminal.strategy": "shared"
}
```

## 🎯 Usage

### Getting Started

1. **Open a workspace** containing an `env.cue` file
2. **Check the status bar** for environment status (bottom left)
3. **View panels** in the Activity Bar under the cuenv icon
4. **Run tasks** from the Tasks panel or using CodeLens

### Environment Panel

The Environment panel displays all environment variables loaded from your `env.cue` file:

- **Masked Variables**: Sensitive variables are automatically masked based on patterns
- **Copy Actions**: Right-click to copy variable names or values
- **Refresh**: Manual refresh button in panel toolbar
- **Toggle Masking**: Temporarily reveal all masked values

### Tasks Panel

The Tasks panel shows all available tasks with rich metadata:

- **Task List**: All tasks with descriptions and dependencies
- **Run Actions**: Click to run tasks or use context menu
- **Terminal Options**: Run in shared terminal or create new terminal
- **Reveal Definition**: Jump to task definition in `env.cue`

### CodeLens Integration

CodeLens provides inline task execution directly in `env.cue` files:

- **Automatic Detection**: "Run Task" buttons appear above task definitions
- **Live Updates**: CodeLens updates when tasks are added/removed
- **Quick Execution**: Click to run tasks without leaving the editor

### Status Bar Integration

The status bar provides quick access to common actions:

- **Status Indicator**: Visual state with icons and tooltips
- **Quick Pick**: Click for action menu (Reload, Open Output, etc.)
- **Multi-root Aware**: Shows status for active editor's workspace folder

## 🔧 Commands

| Command | Description |
|---------|-------------|
| `cuenv.reload` | Reload the current environment |
| `cuenv.viewOutput` | Open the cuenv output channel |
| `cuenv.toggleAutoLoad` | Toggle automatic environment loading |
| `cuenv.runTask` | Run a specific task |
| `cuenv.refreshEnvPanel` | Refresh the environment panel |
| `cuenv.refreshTasksPanel` | Refresh the tasks panel |

## 🐛 Troubleshooting

### Common Issues

#### cuenv Binary Not Found
**Error**: "cuenv binary not found at path: cuenv"

**Solutions**:
1. Install cuenv and ensure it's in your PATH
2. Set the full path in `cuenv.executablePath` setting
3. Restart VSCode after installing cuenv

#### Environment Not Loading
**Symptoms**: Status bar shows "No env.cue file found"

**Solutions**:
1. Ensure `env.cue` exists in workspace root
2. Check file permissions and syntax
3. Enable auto-load in settings: `cuenv.autoLoad.enabled: true`
4. Try manual reload: Command Palette → "cuenv: Reload Environment"

#### Tasks Not Appearing
**Symptoms**: Tasks panel is empty

**Solutions**:
1. Verify tasks are defined in `env.cue` under `tasks` field
2. Check cuenv version supports Task Server Protocol
3. Run `cuenv internal task-protocol --export-json` in terminal to verify
4. Refresh tasks panel manually

#### Masking Not Working
**Symptoms**: Sensitive variables are visible

**Solutions**:
1. Check `cuenv.env.maskPatterns` configuration
2. Verify regex patterns are valid
3. Use case-insensitive patterns: `(?i)secret`
4. Test patterns with online regex tools

### Debug Information

To gather debug information:

1. **Open Output Panel**: View → Output → Select "cuenv"
2. **Enable Debug Logging**: Reload environment to see detailed logs
3. **Check Extension Host**: Help → Toggle Developer Tools → Console
4. **Verify cuenv CLI**: Run `cuenv --version` in terminal

### Performance Tips

- **Large Workspaces**: Increase `cuenv.watch.debounceMs` for better performance
- **Terminal Strategy**: Use `"new"` strategy if shared terminals cause issues
- **Disable Auto-load**: Set `cuenv.autoLoad.enabled: false` for manual control

## 🤝 Contributing

Contributions are welcome! Please see the [cuenv repository](https://github.com/rawkode/cuenv) for development guidelines.

### Development Setup

1. Clone the cuenv repository
2. Navigate to `vscode-extension/` directory
3. Install dependencies: `npm install`
4. Open in VSCode and press F5 to launch extension development host

## 📄 License

This extension is part of the cuenv project. See the main repository for license information.

## 🔗 Related Projects

- [cuenv](https://github.com/rawkode/cuenv) - The main cuenv CLI tool
- [direnv](https://direnv.net/) - Original inspiration for environment management
- [CUE](https://cuelang.org/) - Configuration language used by cuenv