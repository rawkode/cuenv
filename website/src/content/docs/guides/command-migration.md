---
title: Command Migration Guide
description: Migration guide from old documentation to current CLI commands
---

# Command Migration Guide

This guide helps users migrate from commands mentioned in older documentation to the current CLI implementation.

## Command Changes

### Environment Management

| Old Command | New Command | Notes |
|-------------|-------------|-------|
| `cuenv allow` | `cuenv env allow` | Directory approval moved to env subcommand |
| `cuenv deny` | `cuenv env deny` | Directory denial moved to env subcommand |
| `cuenv status` | `cuenv env status` | Status moved to env subcommand |
| `cuenv load` | `cuenv shell load` | Manual loading moved to shell subcommand |
| `cuenv unload` | `cuenv shell unload` | Manual unloading moved to shell subcommand |

### Shell Integration

| Old Command | New Command | Notes |
|-------------|-------------|-------|
| `cuenv init <shell>` | `cuenv shell init <shell>` | Shell initialization moved to shell subcommand |
| `cuenv hook <shell>` | `cuenv shell hook <shell>` | Hook generation moved to shell subcommand |

### Task Execution

| Old Command | New Command | Notes |
|-------------|-------------|-------|
| `cuenv run <task>` | `cuenv task <task>` | Task execution simplified |
| `cuenv run` | `cuenv task` | Task listing simplified |

### Removed Commands

These commands were documented but never implemented:

| Removed Command | Alternative | Notes |
|-----------------|-------------|-------|
| `cuenv dump` | `cuenv env export` | Use export with appropriate format |
| `cuenv prune` | `cuenv env prune` or `cuenv cache cleanup` | State cleanup moved to appropriate subcommands |
| `cuenv remote-cache-server` | Not implemented | Remote cache server is not available |

## Migration Examples

### Shell Setup

**Old:**
```bash
eval "$(cuenv init bash)"
```

**New:**
```bash
eval "$(cuenv shell init bash)"
```

### Directory Approval

**Old:**
```bash
cuenv allow .
```

**New:**
```bash
cuenv env allow .
```

### Task Execution

**Old:**
```bash
cuenv run build
cuenv run -e production deploy
```

**New:**
```bash
cuenv task build
cuenv task deploy -e production
```

### Environment Status

**Old:**
```bash
cuenv status
```

**New:**
```bash
cuenv env status
```

### Environment Loading

**Old:**
```bash
cuenv load
cuenv unload
```

**New:**
```bash
cuenv shell load
cuenv shell unload
```

## Shell Configuration Updates

If you have shell configuration files that reference the old commands, update them:

### ~/.bashrc or ~/.zshrc

**Old:**
```bash
eval "$(cuenv init bash)"
```

**New:**
```bash
eval "$(cuenv shell init bash)"
```

### ~/.config/fish/config.fish

**Old:**
```fish
cuenv init fish | source
```

**New:**
```fish
cuenv shell init fish | source
```

## Alias Compatibility

For backward compatibility, you can create aliases for the old commands:

```bash
# Add to ~/.bashrc or ~/.zshrc
alias cuenv-allow='cuenv env allow'
alias cuenv-deny='cuenv env deny'
alias cuenv-status='cuenv env status'
alias cuenv-load='cuenv shell load'
alias cuenv-unload='cuenv shell unload'
alias cuenv-run='cuenv task'
```

## Script Updates

If you have scripts using the old commands, update them:

**Old script:**
```bash
#!/bin/bash
cuenv allow .
cuenv run test
cuenv run build
cuenv run deploy
```

**New script:**
```bash
#!/bin/bash
cuenv env allow .
cuenv task test
cuenv task build
cuenv task deploy
```

## Configuration File Changes

No changes are needed to your `env.cue` files - the configuration format remains the same. Only the CLI commands have changed.

## Feature Availability

### Available Features

- ✅ Environment variable management
- ✅ Task execution with dependency resolution
- ✅ Shell integration (bash, zsh, fish)
- ✅ Caching system
- ✅ Monorepo support
- ✅ MCP server for Claude Code integration

### Not Implemented

- ❌ Secret resolvers (op://, gcp-secret://)
- ❌ Remote cache server
- ❌ Advanced secret management schemas

If you need these features, consider:
- Using external tools for secret resolution
- Setting up your own remote cache infrastructure
- Using environment variables for secrets in development

## Getting Help

If you encounter issues during migration:

1. Check current command help:
   ```bash
   cuenv --help
   cuenv task --help
   cuenv env --help
   cuenv shell --help
   ```

2. Verify your `env.cue` files are valid:
   ```bash
   cuenv discover --load
   ```

3. Test commands in a new directory:
   ```bash
   mkdir test-cuenv
   cd test-cuenv
   cuenv init
   cuenv env allow .
   cuenv env status
   ```