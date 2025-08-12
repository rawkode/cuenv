# Native Devenv Support Example

This example demonstrates the native Devenv support in cuenv using the `@schema/devenv` schema.

## Usage

```cue
import "github.com/rawkode/cuenv/schema"

hooks: onEnter: [
    schema.#Devenv,
    // Additional hooks...
]
```

## How it Works

The `schema.#Devenv` schema automatically:

- Runs `devenv print-dev-env`
- Sources the environment variables
- Watches `devenv.nix`, `devenv.lock`, and `devenv.yaml` for changes

## Testing

```bash
cd examples/devenv-native
cuenv allow .
cuenv task exec -- echo $NODE_VERSION
```
