---
title: Nix Integration
description: Integrate cuenv with nix develop for reproducible development environments
---

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

## Configuration Example

```cue
package env

import "github.com/rawkode/cuenv"

// Source nix develop environment
hooks: {
    onEnter: {
        command: "nix"
        args: ["develop", "--print-dev-env"]
        source: true
    }
}

// Define environment variables
env: cuenv.#Env & {
    APP_NAME: "my-rust-app"
    RUST_LOG: "debug"
    DATABASE_URL: "sqlite://./app.db"

    // Environment-specific overrides
    environment: {
        production: {
            RUST_LOG: "info"
            DATABASE_URL: "postgres://prod.example.com/db"
        }
    }
}

// Define development tasks
tasks: {
    "build": {
        description: "Build the Rust application"
        command: "cargo build --release"
        cache: true
        inputs: ["src/**", "Cargo.toml", "Cargo.lock"]
        outputs: ["target/release/**"]
    }

    "test": {
        description: "Run tests"
        command: "cargo test"
        dependencies: ["build"]
        cache: true
        inputs: ["src/**", "tests/**", "Cargo.toml"]
    }

    "dev": {
        description: "Start development server"
        command: "cargo run"
        dependencies: ["build"]
    }
}
```

## flake.nix Example

```nix
{
  description = "Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          rust-bin.stable.latest.default
          pkg-config
          openssl
          sqlite
        ];

        shellHook = ''
          export RUST_BACKTRACE=1
          echo "🦀 Rust development environment loaded"
        '';
      };
    };
}
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

## Advanced Integration

### Custom Nix Shell

```cue
hooks: {
    onEnter: {
        command: "nix"
        args: ["develop", ".#rust-dev", "--print-dev-env"]
        source: true
    }
}
```

### Conditional Nix Activation

```cue
hooks: {
    onEnter: [
        // Only use nix if flake.nix exists
        if path.exists("flake.nix") {
            command: "nix"
            args: ["develop", "--print-dev-env"]
            source: true
        },
        // Fallback message
        {
            command: "echo"
            args: ["No flake.nix found, using system environment"]
        }
    ]
}
```

### Multiple Nix Shells

```cue
env: cuenv.#Env & {
    environment: {
        rust: {
            hooks: {
                onEnter: {
                    command: "nix"
                    args: ["develop", ".#rust", "--print-dev-env"]
                    source: true
                }
            }
        }

        nodejs: {
            hooks: {
                onEnter: {
                    command: "nix"
                    args: ["develop", ".#nodejs", "--print-dev-env"]
                    source: true
                }
            }
        }
    }
}
```

## Related Guides

- [devenv Integration](/guides/devenv-integration/) - Using devenv instead of nix develop
- [Hooks and Lifecycle](/reference/hooks/) - Environment lifecycle management
- [Task Examples](/guides/task-examples/) - Task automation patterns
