# devenv Integration Example

This example shows how to use cuenv with devenv for seamless Nix development environment integration.

## How it works

1. **Environment Sourcing**: The `onEnter` hook runs `devenv print-dev-env` with `source: true`
2. **Variable Merging**: devenv's environment variables are merged with CUE-defined variables
3. **Precedence**: CUE variables (like `APP_ENV`, `DATABASE_URL`) override devenv variables
4. **Task Integration**: All cuenv tasks inherit the combined environment

## Usage

```bash
# Allow the directory
cuenv allow .

# Run tasks with devenv environment
cuenv run dev     # Start development server
cuenv run test    # Run tests
cuenv run build   # Build with full toolchain

# Execute commands with environment
cuenv exec node --version  # Uses devenv's Node.js
```

## Benefits

- ✅ **Type-safe configuration**: CUE schema validation for all environment setup
- ✅ **Environment precedence**: CUE variables override devenv when needed
- ✅ **Task definitions**: Structured task execution with dependencies
- ✅ **Multi-environment**: Different configs for dev/staging/prod
- ✅ **Automatic activation**: Environment loads automatically with tasks

## Comparison to .envrc

Instead of:

```bash
# .envrc
use devenv
export APP_ENV=development
export DATABASE_URL=postgres://localhost/myapp_dev
```

You get:

```cue
// env.cue - Type-safe, structured configuration
hooks: {
    onEnter: {
        command: "devenv"
        args: ["print-dev-env"]
        source: true
    }
}

env: cuenv.#Env & {
    APP_ENV: "development"
    DATABASE_URL: "postgres://localhost/myapp_dev"
}
```
