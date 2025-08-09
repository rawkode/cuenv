# cuenv

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
       SECRET_KEY: {
           resolver: {
               command: "op"
               args: ["read", "op://vault/item/password"]
           }
       }
   }
   ```

3. **Load** the environment:
   ```bash
   cuenv status  # Check what would be loaded
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

Integrate with password managers and secret stores:

```cue
env: SECRET: {
    resolver: {
        command: "op"
        args: ["read", "op://vault/item/password"]
    }
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

### Security

- Sandboxed command execution
- File system access controls
- Command allowlists
- Audit logging

## Contributing

1. Enter the development environment: `nix develop`
2. Make your changes
3. Run tests: `cargo test --workspace`
4. Format code: `cargo fmt`
5. Check with clippy: `cargo clippy`

## License

MIT License - see [LICENSE](LICENSE) file for details.
