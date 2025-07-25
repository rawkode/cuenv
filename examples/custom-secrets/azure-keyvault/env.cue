package env

import "github.com/rawkode/cuenv"

// Example: Azure Key Vault integration
// Requires: az CLI installed and authenticated
// Usage: az login
env: cuenv.#Env & {
	// Secret from Azure Key Vault
	DATABASE_PASSWORD: {
		resolver: {
			command: "az"
			args: [
				"keyvault", "secret", "show",
				"--vault-name", "myapp-keyvault",
				"--name", "database-password",
				"--query", "value",
				"--output", "tsv"
			]
		}
	}

	// API key with specific version
	API_KEY: {
		resolver: {
			command: "az"
			args: [
				"keyvault", "secret", "show",
				"--vault-name", "myapp-keyvault",
				"--name", "external-api-key",
				"--version", "latest",
				"--query", "value",
				"--output", "tsv"
			]
		}
	}

	// Certificate from Key Vault (base64 encoded)
	TLS_CERTIFICATE: {
		resolver: {
			command: "az"
			args: [
				"keyvault", "certificate", "download",
				"--vault-name", "myapp-keyvault",
				"--name", "tls-cert",
				"--encoding", "base64",
				"--file", "/dev/stdout"
			]
		}
	}

	// Environment-specific vaults
	environment: {
		development: {
			AZURE_KEYVAULT_NAME: "myapp-dev-kv"
			DATABASE_PASSWORD: {
				resolver: {
					command: "az"
					args: [
						"keyvault", "secret", "show",
						"--vault-name", "myapp-dev-kv",
						"--name", "database-password",
						"--query", "value",
						"--output", "tsv"
					]
				}
			}
		}
		production: {
			AZURE_KEYVAULT_NAME: "myapp-prod-kv"
			DATABASE_PASSWORD: {
				resolver: {
					command: "az"
					args: [
						"keyvault", "secret", "show",
						"--vault-name", "myapp-prod-kv",
						"--name", "database-password",
						"--query", "value",
						"--output", "tsv"
					]
				}
			}
		}
	}

	// Regular environment variables
	APP_NAME: "azure-keyvault-example"
	AZURE_SUBSCRIPTION_ID: "12345678-1234-1234-1234-123456789012"
}