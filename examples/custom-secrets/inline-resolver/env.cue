package examples

// Example: Custom command resolver for HashiCorp Vault
env: {
	// Inline custom resolver
	DATABASE_PASSWORD: {
		resolver: {
			command: "vault"
			args: ["kv", "get", "-field=password", "secret/myapp/database"]
		}
	}

	// Regular environment variables
	APP_NAME: "custom-resolver-example"
}
