---
title: GitHub Actions Integration
description: Install and use cuenv in GitHub Actions workflows with the setup-cuenv action
---

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

### Complete CI/CD Pipeline

```yaml
name: Complete CI/CD

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup cuenv
        uses: rawkode/cuenv/github/action/setup-cuenv@main
        with:
          version: latest

      - name: Allow cuenv directory
        run: cuenv allow .

      - name: Run linting
        run: cuenv run lint

      - name: Run tests
        run: cuenv run test

      - name: Build application
        run: cuenv run build

  deploy:
    needs: test
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4

      - name: Setup cuenv
        uses: rawkode/cuenv/github/action/setup-cuenv@main

      - name: Deploy to production
        run: cuenv run deploy
        env:
          DEPLOY_ENV: production
```

### Matrix builds with cuenv

```yaml
name: Matrix Build

on: [push, pull_request]

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macOS-latest]
        cuenv-version: [v0.2.7, latest]

    steps:
      - uses: actions/checkout@v4

      - name: Setup cuenv
        uses: rawkode/cuenv/github/action/setup-cuenv@main
        with:
          version: ${{ matrix.cuenv-version }}

      - name: Run build
        run: cuenv run build
```

## Platform Support

This action supports:

- Linux (x86_64, aarch64)
- macOS (x86_64, aarch64)

## Best Practices

### Caching

Use GitHub Actions caching to speed up builds:

```yaml
- name: Cache cuenv builds
  uses: actions/cache@v3
  with:
    path: ~/.cache/cuenv
    key: ${{ runner.os }}-cuenv-${{ hashFiles('**/env.cue') }}
    restore-keys: |
      ${{ runner.os }}-cuenv-
```

### Security

For production deployments, pin to specific versions:

```yaml
- name: Setup cuenv
  uses: rawkode/cuenv/github/action/setup-cuenv@v1.0.0 # Pin to specific version
  with:
    version: v0.2.7 # Pin cuenv version too
```

### Environment Variables

Pass secrets and configuration through GitHub Actions:

```yaml
- name: Deploy application
  run: cuenv run deploy
  env:
    DATABASE_URL: ${{ secrets.DATABASE_URL }}
    API_KEY: ${{ secrets.API_KEY }}
    DEPLOY_ENV: production
```

## Troubleshooting

### Permission Issues

If you encounter permission issues, ensure the install directory is writable:

```yaml
- name: Setup cuenv
  uses: rawkode/cuenv/github/action/setup-cuenv@main
  with:
    install-dir: ${{ github.workspace }}/bin
```

### Path Issues

Make sure cuenv is in PATH after installation:

```yaml
- name: Setup cuenv
  uses: rawkode/cuenv/github/action/setup-cuenv@main
  id: cuenv

- name: Verify installation
  run: |
    echo "Cuenv installed at: ${{ steps.cuenv.outputs.path }}"
    echo "Cuenv version: ${{ steps.cuenv.outputs.version }}"
    which cuenv
    cuenv --version
```

## License

This action is part of the cuenv project and follows the same license.
