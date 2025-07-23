package cuenv

// Define secret type schemas with resolver configurations
#SecretRef: {
	resolver: {
		command: string
		args: [...string]
	}
	...
}

#OnePasswordRef: #SecretRef & {
	vault: string
	item: string
	field?: string
	section?: string
	
	resolver: {
		command: "op"
		args: ["read", _opUri]
	}
	
	_opUri: string
	if section != _|_ && field != _|_ {
		_opUri: "op://\(vault)/\(item)/\(section)/\(field)"
	}
	if section == _|_ && field != _|_ {
		_opUri: "op://\(vault)/\(item)/\(field)"
	}
	if section == _|_ && field == _|_ {
		_opUri: "op://\(vault)/\(item)"
	}
}

// Legacy compatibility - map old path format to new format
#OnePassword: #SecretRef & {
	vault: string
	path: string
	
	resolver: {
		command: "op"
		args: ["read", "op://\(vault)/\(path)"]
	}
}

#GcpSecret: #SecretRef & {
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

// Base configuration
DATABASE_NAME: "application"
DATABASE_URL: "postgresql://localhost:5432/\(DATABASE_NAME)"
API_BASE_URL: "https://api.example.com/v1"

PORT: 8080
MAX_CONNECTIONS: 100
DEBUG: true
LOG_LEVEL: "info"

// Secrets using structured types
AWS_ACCESS_KEY: #OnePasswordRef & { 
	vault: "Personal"
	item: "aws"
	section: "api-tokens"
	field: "access-key"
} @capability("aws")

AWS_SECRET_KEY: #OnePasswordRef & {
	vault: "Personal"
	item: "aws"
	section: "api-tokens" 
	field: "secret-key"
} @capability("aws")

// GCP secret with version
DATABASE_PASSWORD: #GcpSecret & {
	project: "my-project"
	secret: "db-password"
	version: "latest"
}

// GCP secret without version (defaults to latest)
API_KEY: #GcpSecret & {
	project: "my-project"
	secret: "api-key"
}

// Additional secrets using structured types
STRIPE_KEY: #OnePasswordRef & {
	vault: "Work"
	item: "stripe"
	field: "test-key"
} @capability("stripe")

GITHUB_TOKEN: #GcpSecret & {
	project: "my-project"
	secret: "github-token"
}

// Environment-specific configurations
environment: {
	production: {
		DATABASE_URL: "postgresql://production:5432/\(DATABASE_NAME)"
		
		// Override with production secrets
		AWS_ACCESS_KEY: #OnePasswordRef & { 
			vault: "Production"
			item: "aws"
			section: "api-tokens"
			field: "access-key"
		} @capability("aws")
		
		AWS_SECRET_KEY: #OnePasswordRef & {
			vault: "Production"
			item: "aws"
			section: "api-tokens"
			field: "secret-key"
		} @capability("aws")
		
		DATABASE_PASSWORD: #GcpSecret & {
			project: "prod-project"
			secret: "db-password"
			version: "2"
		}
		
		// Override with production stripe key
		STRIPE_KEY: #OnePasswordRef & {
			vault: "Production"
			item: "stripe"
			field: "live-key"
		} @capability("stripe")
	}
	
	staging: {
		DATABASE_URL: "postgresql://staging:5432/\(DATABASE_NAME)_staging"
		
		// Staging uses different vault
		AWS_ACCESS_KEY: #OnePasswordRef & { 
			vault: "Staging"
			item: "aws"
			field: "access-key"
		} @capability("aws")
		
		DATABASE_PASSWORD: #GcpSecret & {
			project: "staging-project"
			secret: "db-password"
		}
	}
}

// Command capability mappings
Commands: {
	terraform: capabilities: ["aws", "gcp"]
	aws: capabilities: ["aws"]
	gcloud: capabilities: ["gcp"]
	stripe: capabilities: ["stripe"]
}
