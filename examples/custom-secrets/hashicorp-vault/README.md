# HashiCorp Vault Integration

This example shows how to integrate cuenv with HashiCorp Vault for secret management.

## Prerequisites

1. Install [Vault CLI](https://developer.hashicorp.com/vault/docs/install)
2. Authenticate with your Vault server:
   ```bash
   vault auth -method=userpass username=myuser
   # or
   vault auth -method=oidc
   # or
   export VAULT_TOKEN="your-token"
   ```

## Configuration

The `env.cue` file demonstrates:

- **Key-Value secrets**: Using `vault kv get` for static secrets
- **Dynamic secrets**: Using `vault read` for AWS credentials  
- **Environment-specific paths**: Different Vault paths for dev/prod
- **Field extraction**: Using `-field` to get specific values

## Usage

```bash
# Set environment
export VAULT_ADDR="https://vault.company.com"
export VAULT_NAMESPACE="myteam"  # If using Vault Enterprise

# Run application with resolved secrets
cuenv run -- my-application

# Or with specific environment
cuenv run -e production -- my-application
```

## Vault Setup Example

```bash
# Enable KV secrets engine
vault secrets enable -path=secret kv-v2

# Store development secrets
vault kv put secret/dev/database password="dev-db-password"

# Store production secrets  
vault kv put secret/prod/database password="secure-prod-password"

# Store API keys
vault kv put secret/external/stripe api_key="sk_test_..."

# Enable AWS secrets engine for dynamic credentials
vault secrets enable aws
vault write aws/config/root \
    access_key="AKIA..." \
    secret_key="..." \
    region="us-east-1"
```

## Security Notes

- Secrets are automatically obfuscated in command output
- Vault tokens should be short-lived when possible
- Use Vault's dynamic secrets for cloud provider credentials
- Consider using AppRole or other machine authentication methods for production