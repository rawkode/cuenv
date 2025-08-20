# cuenv

[![Build Status](https://github.com/rawkode/cuenv/workflows/ci/badge.svg)](https://github.com/rawkode/cuenv/actions)
[![codecov](https://codecov.io/gh/rawkode/cuenv/branch/main/graph/badge.svg)](https://codecov.io/gh/rawkode/cuenv)
[![FlakeHub](https://img.shields.io/endpoint?url=https://flakehub.com/f/rawkode/cuenv/badge)](https://flakehub.com/flake/rawkode/cuenv)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A direnv alternative that uses CUE (Configure, Unify, Execute) files for type-safe environment configuration.

## What is cuenv?

cuenv provides:

- **Type-safe environment configuration** using CUE language
- **Shell integration** for bash, zsh, fish, and more
- **Task execution** with dependency management and caching
- **Security features** including sandboxing and access controls
- **Monorepo support** with cross-package dependencies

## Quick Start

### Installation

**Using Nix (Recommended):**

```bash
nix profile install github:rawkode/cuenv
```

**From source:**

```bash
# Requires Nix for the development environment
git clone https://github.com/rawkode/cuenv
cd cuenv
nix develop
cargo build --release
```

### Basic Usage

1. **Initialize** in your project directory:

   ```bash
   cuenv init
   ```

2. **Edit** the generated `env.cue` file:

   ```cue
   package env

   env: {
       NODE_ENV: "development"
       API_URL: "http://localhost:3000"
   }
   ```

3. **Allow and load** the environment:
   ```bash
   cuenv env allow  # Allow this directory
   cuenv env status  # Check what would be loaded
   ```

## Development

All development must be done within the Nix development shell:

```bash
# Enter development environment
nix develop

# Build the workspace
cargo build --workspace

# Run tests
cargo test --workspace

# Run the binary
cargo run --bin cuenv -- --help

# Format and lint
cargo fmt
cargo clippy

# Build release version
cargo build --release
```

## Project Structure

This is a Rust workspace with the following crates:

- **cuenv-cli** - Main binary and CLI interface
- **cuenv-core** - Core types, errors, and event system
- **cuenv-config** - CUE configuration parsing
- **cuenv-env** - Environment management
- **cuenv-task** - Task execution engine
- **cuenv-cache** - Caching system
- **cuenv-security** - Security features and validation
- **cuenv-shell** - Shell integrations
- **cuenv-tui** - Terminal UI components
- **cuenv-hooks** - Hook management
- **cuenv-utils** - Shared utilities
- **cuenv-libcue-ffi-bridge** - Go/CUE FFI bridge

## Configuration

Create an `env.cue` file in your project root:

```cue
package env

// Environment variables
env: {
    NODE_ENV: "development"
    DATABASE_URL: "postgresql://localhost/myapp"

    // Secret resolution
    SECRET_KEY: {
        resolver: {
            command: "op"
            args: ["read", "op://vault/secret"]
        }
    }
}

// Tasks with dependencies
tasks: {
    install: {
        command: "npm install"
    }

    build: {
        command: "npm run build"
        dependencies: ["install"]
        cache: {
            enabled: true
            inputs: ["package.json", "package-lock.json"]
            outputs: ["dist/"]
        }
    }
}
```

## Features

### Type Safety

CUE provides compile-time validation of your environment configuration.

### Secret Management

Integrate with external secret systems:

```cue
env: SECRET: {
    // Custom secret resolution via external commands
    command: "op"
    args: ["read", "op://vault/item/password"]
}
```

### Task Execution

Define tasks with dependencies and caching:

```cue
tasks: {
    test: {
        command: "npm test"
        dependencies: ["build"]
    }
}
```

Execute tasks:

```bash
cuenv task test
```

### Security

- Sandboxed command execution
- File system access controls
- Command allowlists
- Audit logging

### Claude Code Integration (MCP)

cuenv provides built-in MCP (Model Context Protocol) server support for seamless integration with Claude Code:

```bash
# Start MCP server for Claude Code (read-only)
cuenv mcp

# Enable task execution
cuenv mcp --allow-exec
```

**Configure Claude Code (`.mcp.json`):**

```json
{
	"servers": {
		"cuenv": {
			"command": "cuenv",
			"args": ["mcp", "--allow-exec"],
			"type": "stdio",
			"description": "cuenv environment and task management"
		}
	}
}
```

**Available MCP tools:**

- `cuenv.list_env_vars` - List environment variables
- `cuenv.get_env_var` - Get specific variable value
- `cuenv.list_tasks` - List available tasks
- `cuenv.get_task` - Get task details
- `cuenv.run_task` - Execute tasks (requires --allow-exec)
- `cuenv.check_directory` - Validate directory configuration

This allows Claude Code to programmatically manage environments and execute tasks in your projects.

### Starship Integration

cuenv provides real-time hook progress tracking that integrates with [Starship prompt](https://starship.rs/):

**Add to your `starship.toml`:**

```toml
[custom.cuenv_hooks]
command = "cuenv env status --hooks --format=starship"
when = """ test -n "$CUENV_DIR" """
format = "$output"
disabled = false
```

**Features:**

- **Real-time progress**: Shows `⏳ 2/3 hooks (15s)` while hooks run
- **Success indicator**: Displays `✅ Hooks ready` when complete
- **Error alerts**: Shows `⚠️ 1 hook failed` on failures
- **Auto-hide**: Disappears 5 seconds after completion
- **Non-intrusive**: Only visible during hook execution

**Advanced configuration:**

```toml
# Verbose mode - shows individual hook details
[custom.cuenv_hooks_verbose]
command = "cuenv env status --hooks --format=starship --verbose"
when = """ test -n "$CUENV_DIR" """
format = "$output"  # Shows: 🔄 nix develop (12s)
```

See [examples/starship-module](examples/starship-module/) for more configuration options and customization examples.

## Contributing

1. Enter the development environment: `nix develop`
2. Make your changes
3. Run tests: `cargo test --workspace`
4. Format code: `cargo fmt`
5. Check with clippy: `cargo clippy`

## License

MIT License - see [LICENSE](LICENSE) file for details.
