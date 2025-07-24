---
title: Introduction to cuenv
description: Learn about cuenv and why it's the perfect environment management tool for modern development
---

cuenv is a modern alternative to direnv that leverages the power of CUE (Configure, Unify, Execute) for type-safe environment configuration. It provides automatic environment loading, hierarchical configuration, and built-in secret management.

## What is cuenv?

cuenv automatically loads environment variables from `env.cue` files as you navigate through your filesystem. Unlike traditional shell-based solutions, cuenv uses CUE's powerful type system to validate your configuration at load time, preventing common errors before they reach your application.

## Key Benefits

### Type Safety
With CUE, you get compile-time validation of your environment configuration. No more runtime surprises from typos or incorrect values.

### Hierarchical Configuration
cuenv automatically loads parent directory configurations, making it easy to share common settings across multiple projects while allowing project-specific overrides.

### Security First
Built-in integration with secret managers (1Password, GCP Secrets) and automatic obfuscation prevents accidental exposure of sensitive data.

### Cross-Platform
Works seamlessly across Linux, macOS, and Windows with native shell integrations for Bash, Zsh, and Fish.

## How It Works

1. **Create an `env.cue` file** in your project directory
2. **Define your environment variables** using CUE syntax
3. **Navigate to the directory** and cuenv automatically loads the environment
4. **Leave the directory** and the environment is restored

## Comparison with direnv

| Feature | cuenv | direnv |
|---------|-------|--------|
| Configuration Language | CUE (type-safe) | Shell scripts |
| Validation | Compile-time | Runtime |
| Secret Management | Built-in | Manual |
| Allow Required | No | Yes |
| Hierarchical Loading | Yes | Limited |
| Cross-Platform | Yes | Limited Windows support |

## Next Steps

- [Install cuenv](/installation/) and set up your shell
- Follow the [Quick Start](/quickstart/) guide
- Learn about the [CUE file format](/guides/cue-format/)