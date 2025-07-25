package env

import "github.com/rawkode/cuenv"

// Example: HashiCorp Vault secret resolution
// Requires: vault CLI installed and authenticated
// Usage: vault auth -method=userpass username=myuser
env: cuenv.#Env & {
	// Database credentials from Vault
	DATABASE_PASSWORD: {
		resolver: {
			command: "vault"
			args: [
				"kv", "get", "-field=password",
				"secret/myapp/database"
			]
		}
	}

	// API key from different Vault path
	API_KEY: {
		resolver: {
			command: "vault"
			args: [
				"kv", "get", "-field=api_key", 
				"secret/external/stripe"
			]
		}
	}

	// JWT signing key from Vault
	JWT_SIGNING_KEY: {
		resolver: {
			command: "vault"
			args: [
				"kv", "get", "-field=private_key",
				"secret/myapp/jwt"
			]
		}
	}

	// AWS credentials from Vault dynamic secrets
	AWS_ACCESS_KEY_ID: {
		resolver: {
			command: "vault"
			args: [
				"read", "-field=access_key",
				"aws/creds/my-role"
			]
		}
	}

	AWS_SECRET_ACCESS_KEY: {
		resolver: {
			command: "vault"
			args: [
				"read", "-field=secret_key",
				"aws/creds/my-role"
			]
		}
	}

	// Environment-specific secrets
	environment: {
		development: {
			VAULT_NAMESPACE: "development"
			DATABASE_PASSWORD: {
				resolver: {
					command: "vault"
					args: [
						"kv", "get", "-field=password",
						"secret/dev/database"
					]
				}
			}
		}
		production: {
			VAULT_NAMESPACE: "production"
			DATABASE_PASSWORD: {
				resolver: {
					command: "vault"
					args: [
						"kv", "get", "-field=password",
						"secret/prod/database"
					]
				}
			}
		}
	}

	// Regular environment variables
	APP_NAME: "vault-example"
	DATABASE_URL: "postgres://user:\(DATABASE_PASSWORD)@localhost/myapp"
}