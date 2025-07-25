package env

import "github.com/rawkode/cuenv"

// Example: Custom command resolver for HashiCorp Vault
env: cuenv.#Env & {
	// Inline custom resolver
	DATABASE_PASSWORD: cuenv.#Secret & {
		resolver: {
			command: "vault"
			args: ["kv", "get", "-field=password", "secret/myapp/database"]
		}
	}

	// Regular environment variables
	APP_NAME: "custom-resolver-example"
}