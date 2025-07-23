package cuenv

// Import secret types from CUE registry (future)
// import "registry.cue.works/secrets/onepassword"
// import "registry.cue.works/secrets/gcp"

// For now, define them locally to demonstrate the pattern
#SecretRef: {
	// Resolver configuration tells cuenv how to resolve this secret
	resolver: {
		command: string
		args: [...string]
	}
	// Additional fields specific to the secret type
	...
}

#OnePasswordRef: #SecretRef & {
	vault: string
	item: string
	field?: string
	section?: string
	
	// Build resolver command based on fields
	resolver: {
		command: "op"
		args: ["read", _opUri]
	}
	
	// Construct the URI from components
	_opUri: string
	if section != _|_ && field != _|_ {
		_opUri: "op://\(vault)/\(item)/\(section)/\(field)"
	}
	if section == _|_ && field != _|_ {
		_opUri: "op://\(vault)/\(item)/\(field)"
	}
	if section == _|_ && field == _|_ {
		// When no field specified, need to get item and extract password
		// This would need special handling in the resolver
		_opUri: "op://\(vault)/\(item)"
	}
}

#GcpSecretRef: #SecretRef & {
	project: string
	secret: string
	version: string | *"latest"
	
	resolver: {
		command: "gcloud"
		args: [
			"secrets", "versions", "access", version,
			"--secret", secret,
			"--project", project,
		]
	}
}

// Custom secret provider example
#VaultRef: #SecretRef & {
	path: string
	field: string
	
	resolver: {
		command: "vault"
		args: ["kv", "get", "-field=\(field)", path]
	}
}

// Application configuration
DATABASE_NAME: "myapp"
PORT: 8080

// Secrets using the new structured format
DATABASE_PASSWORD: #OnePasswordRef & {
	vault: "Production"
	item: "database"
	field: "password"
}

AWS_ACCESS_KEY: #OnePasswordRef & {
	vault: "DevOps"
	item: "aws"
	section: "production"
	field: "access-key"
} @capability("aws")

API_KEY: #GcpSecretRef & {
	project: "my-project"
	secret: "api-key"
	version: "3"
}

// Example with custom Vault provider
ENCRYPTION_KEY: #VaultRef & {
	path: "secret/data/myapp/prod"
	field: "encryption_key"
}

// Environment overrides
environment: {
	staging: {
		DATABASE_PASSWORD: #OnePasswordRef & {
			vault: "Staging"
			item: "database"
			field: "password"
		}
		
		API_KEY: #GcpSecretRef & {
			project: "staging-project"
			secret: "api-key"
			// version defaults to "latest"
		}
	}
}
