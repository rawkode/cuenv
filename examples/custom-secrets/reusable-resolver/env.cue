package env

import "github.com/rawkode/cuenv"

// Define a reusable resolver for HashiCorp Vault
#VaultRef: cuenv.#Secret & {
	path:  string
	field: string
	resolver: {
		command: "vault"
		args: ["kv", "get", "-field=\(field)", path]
	}
}

// Example: Using the reusable VaultRef resolver
env: cuenv.#Env & {
	// Use the reusable resolver
	DATABASE_PASSWORD: #VaultRef & {
		path:  "secret/myapp/database"
		field: "password"
	}

	API_KEY: #VaultRef & {
		path:  "secret/myapp/external"
		field: "api_key"
	}

	// Regular environment variables
	APP_NAME: "reusable-resolver-example"
}
