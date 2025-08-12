package env

// Define a reusable resolver for HashiCorp Vault
#VaultRef: {
	path:  string
	field: string
	resolver: {
		command: "vault"
		args: ["kv", "get", "-field=\(field)", path]
	}
}

// Example: Using the reusable VaultRef resolver
env: {
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
