# Azure Key Vault Integration

This example demonstrates how to integrate cuenv with Azure Key Vault for secret management.

## Prerequisites

1. Install [Azure CLI](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli)
2. Authenticate with Azure:
   ```bash
   az login
   ```

## Configuration

The `env.cue` file demonstrates:

- **Simple secrets**: Using `az keyvault secret show`
- **Versioned secrets**: Specifying specific secret versions
- **Certificates**: Downloading certificates from Key Vault
- **Environment-specific vaults**: Different Key Vaults for different environments

## Usage

```bash
# Verify Azure authentication
az account show

# Set your default subscription if needed
az account set --subscription "your-subscription-id"

# Run application with resolved secrets
cuenv run -- my-application

# Use specific environment
cuenv run -e production -- my-application
```

## Azure Key Vault Setup

```bash
# Create resource group
az group create --name myapp-rg --location eastus

# Create development Key Vault
az keyvault create \
    --name myapp-dev-kv \
    --resource-group myapp-rg \
    --location eastus

# Create production Key Vault
az keyvault create \
    --name myapp-prod-kv \
    --resource-group myapp-rg \
    --location eastus

# Add secrets
az keyvault secret set \
    --vault-name myapp-dev-kv \
    --name database-password \
    --value "dev-password"

az keyvault secret set \
    --vault-name myapp-prod-kv \
    --name database-password \
    --value "super-secure-prod-password"

# Upload certificate
az keyvault certificate import \
    --vault-name myapp-prod-kv \
    --name tls-cert \
    --file certificate.pfx
```

## Required Permissions

Your Azure identity needs the following permissions on the Key Vault:

- **Secret permissions**: Get, List
- **Certificate permissions**: Get, List (if using certificates)

You can assign these via access policies:

```bash
# Get your object ID
OBJECT_ID=$(az ad signed-in-user show --query objectId --output tsv)

# Assign secret permissions
az keyvault set-policy \
    --name myapp-dev-kv \
    --object-id $OBJECT_ID \
    --secret-permissions get list

az keyvault set-policy \
    --name myapp-prod-kv \
    --object-id $OBJECT_ID \
    --secret-permissions get list \
    --certificate-permissions get list
```