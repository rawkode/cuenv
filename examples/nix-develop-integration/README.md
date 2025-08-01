# nix develop Integration Example

This example demonstrates cuenv integration with `nix develop` for Rust development.

## How it works

1. **Nix Environment**: `nix develop --print-dev-env` provides the development toolchain
2. **Environment Sourcing**: cuenv sources the nix environment via the `source: true` hook
3. **Variable Integration**: Nix variables (PATH, PKG_CONFIG_PATH, etc.) are merged with CUE config
4. **Task Execution**: All tasks run with the combined nix + cuenv environment

## Usage

```bash
# Allow the directory
cuenv allow .

# Tasks automatically use nix environment
cuenv run build   # Uses nix-provided Rust toolchain
cuenv run test    # Runs with nix dependencies
cuenv run dev     # Development server with full environment

# Execute commands with nix + cuenv environment
cuenv exec rustc --version    # Uses nix Rust compiler
cuenv exec cargo check        # Uses nix cargo with custom env vars
```

## Environment Precedence

- **Nix variables**: PATH, LD_LIBRARY_PATH, PKG_CONFIG_PATH (from nix develop)
- **CUE variables**: APP_NAME, RUST_LOG, DATABASE_URL (override nix if conflicts)
- **Combined result**: Best of both - nix toolchain + cuenv configuration

## Benefits over direnv + nix-direnv

### With direnv + nix-direnv:

```bash
# .envrc
use flake
export APP_NAME=my-rust-app
export RUST_LOG=debug
```

### With cuenv:

```cue
// env.cue - Structured, type-safe configuration
hooks: {
    onEnter: {
        command: "nix"
        args: ["develop", "--print-dev-env"]
        source: true
    }
}

env: cuenv.#Env & {
    APP_NAME: "my-rust-app"
    RUST_LOG: "debug"
    DATABASE_URL: "sqlite://./app.db"
}

tasks: {
    "build": { command: "cargo build --release", cache: true }
    "test": { command: "cargo test", dependencies: ["build"] }
}
```

**Advantages:**

- ✅ **Type safety**: CUE schema validation
- ✅ **Task management**: Built-in task dependencies and caching
- ✅ **Multi-environment**: Easy dev/staging/prod configurations
- ✅ **Explicit control**: Clear precedence and merging rules
