# Custom Secret Resolvers Examples

This directory demonstrates how to create custom secret resolvers for different secret management systems using cuenv's flexible resolver framework.

## Overview

cuenv supports custom command-based secret resolvers that can integrate with any secret management system via the `#Resolver` schema. This allows you to:

- Integrate with enterprise secret management systems
- Use custom authentication mechanisms
- Support legacy secret storage systems
- Create specialized secret transformation logic

## Examples

- **`hashicorp-vault/`** - HashiCorp Vault integration
- **`aws-secrets/`** - AWS Secrets Manager integration  
- **`azure-keyvault/`** - Azure Key Vault integration
- **`sops/`** - Mozilla SOPS file-based secrets
- **`pass/`** - Unix password manager integration
- **`bitwarden/`** - Bitwarden CLI integration
- **`custom-transform/`** - Custom transformation and validation

## How Custom Resolvers Work

1. Define a resolver using the `#Resolver` schema in your CUE environment
2. Specify the command and arguments to execute for secret resolution
3. cuenv executes the command and captures the output as the secret value
4. The resolved secret is obfuscated in logs and command output

## Basic Structure

```cue
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    MY_SECRET: {
        resolver: {
            command: "your-secret-command"
            args: ["--get", "secret-name"]
        }
    }
}
```

## Usage

To use these examples:

```bash
cd examples/custom-secrets/hashicorp-vault
cuenv run -- your-application
```

The resolver will be executed automatically when using `cuenv run`, and the secret will be available in your application environment.