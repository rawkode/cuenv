# Setup cuenv GitHub Action

This action installs [cuenv](https://github.com/rawkode/cuenv) in your GitHub Actions workflow.

## Usage

### Basic usage (installs latest version)

```yaml
- uses: rawkode/cuenv/github/action/setup-cuenv@main
```

### Specify a version

```yaml
- uses: rawkode/cuenv/github/action/setup-cuenv@main
  with:
    version: v0.2.7
```

### Custom installation directory

```yaml
- uses: rawkode/cuenv/github/action/setup-cuenv@main
  with:
    install-dir: /usr/local/bin
```

## Inputs

| Name          | Description                                  | Required | Default            |
| ------------- | -------------------------------------------- | -------- | ------------------ |
| `version`     | Version of cuenv to install (e.g., `v0.2.7`) | No       | `latest`           |
| `install-dir` | Directory to install cuenv                   | No       | `$HOME/.local/bin` |

## Outputs

| Name      | Description                        |
| --------- | ---------------------------------- |
| `version` | The installed version of cuenv     |
| `path`    | The path where cuenv was installed |

## Example workflows

### Basic usage

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup cuenv
        uses: rawkode/cuenv/github/action/setup-cuenv@main
        with:
          version: v0.2.7

      - name: Run cuenv
        run: |
          cuenv --version
          cuenv run -- echo "Hello from cuenv!"
```

### Building a project with cuenv

```yaml
name: Build with cuenv

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup cuenv
        uses: rawkode/cuenv/github/action/setup-cuenv@main

      - name: Install Nix (if using cuenv.nix)
        uses: cachix/install-nix-action@v27
        if: ${{ hashFiles('cuenv.nix') != '' }}

      - name: Build project
        run: |
          cuenv run -- cargo build --release
          cuenv run -- cargo test
```

See [example-workflow.yml](./example-workflow.yml) for a complete example.

## Platform Support

This action supports:

- Linux (x86_64, aarch64)
- macOS (x86_64, aarch64)

## License

This action is part of the cuenv project and follows the same license.
