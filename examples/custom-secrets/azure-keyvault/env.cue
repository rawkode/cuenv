package env

import "github.com/rawkode/cuenv"

// Example: Azure Key Vault integration using reusable AzureKeyVaultRef
// Requires: az CLI installed and authenticated
env: cuenv.#Env & {
	// Database password from Azure Key Vault
	DATABASE_PASSWORD: cuenv.#AzureKeyVaultRef & {
		vaultName:  "myapp-keyvault"
		secretName: "database-password"
	}

	// API key from Key Vault
	API_KEY: cuenv.#AzureKeyVaultRef & {
		vaultName:  "myapp-keyvault"
		secretName: "external-api-key"
	}

	// Certificate from Key Vault (base64 encoded)
	TLS_CERTIFICATE: cuenv.#AzureKeyVaultCertRef & {
		vaultName: "myapp-keyvault"
		certName:  "tls-cert"
		format:    "base64"
	}

	// Environment-specific secrets
	environment: {
		development: {
			AZURE_KEYVAULT_NAME: "myapp-dev-kv"
			DATABASE_PASSWORD: cuenv.#AzureKeyVaultRef & {
				vaultName:  "myapp-dev-kv"
				secretName: "database-password"
			}
		}
		production: {
			AZURE_KEYVAULT_NAME: "myapp-prod-kv"
			DATABASE_PASSWORD: cuenv.#AzureKeyVaultRef & {
				vaultName:  "myapp-prod-kv"
				secretName: "database-password"
			}
		}
	}

	// Regular environment variables
	APP_NAME: "azure-keyvault-example"
	AZURE_SUBSCRIPTION_ID: "12345678-1234-1234-1234-123456789012"
}