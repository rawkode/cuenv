package env

import "github.com/rawkode/cuenv"

// Example: HashiCorp Vault secret resolution using reusable VaultRef
// Requires: vault CLI installed and authenticated
env: cuenv.#Env & {
	// Database credentials from Vault KV store
	DATABASE_PASSWORD: cuenv.#VaultRef & {
		path:  "secret/myapp/database"
		field: "password"
	}

	// API key from different Vault path
	API_KEY: cuenv.#VaultRef & {
		path:  "secret/external/stripe"
		field: "api_key"
	}

	// JWT signing key from Vault
	JWT_SIGNING_KEY: cuenv.#VaultRef & {
		path:  "secret/myapp/jwt"
		field: "private_key"
	}

	// AWS credentials from Vault dynamic secrets
	AWS_ACCESS_KEY_ID: cuenv.#VaultDynamicRef & {
		path:  "aws/creds/my-role"
		field: "access_key"
	}

	AWS_SECRET_ACCESS_KEY: cuenv.#VaultDynamicRef & {
		path:  "aws/creds/my-role"
		field: "secret_key"
	}

	// Environment-specific secrets
	environment: {
		development: {
			VAULT_NAMESPACE: "development"
			DATABASE_PASSWORD: cuenv.#VaultRef & {
				path:  "secret/dev/database"
				field: "password"
			}
		}
		production: {
			VAULT_NAMESPACE: "production"
			DATABASE_PASSWORD: cuenv.#VaultRef & {
				path:  "secret/prod/database"
				field: "password"
			}
		}
	}

	// Regular environment variables
	APP_NAME: "vault-example"
	DATABASE_URL: "postgres://user:\(DATABASE_PASSWORD)@localhost/myapp"
}