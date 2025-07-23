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

DATABASE_NAME: "application"
DATABASE_URL: "postgresql://localhost:5432/\(DATABASE_NAME)"
API_BASE_URL: "https://api.example.com/v1"

PORT: 8080
MAX_CONNECTIONS: 100

DEBUG: true
ENABLE_METRICS: false

LOG_LEVEL: "info"

AWS_ACCESS_KEY: #OnePasswordRef & { 
	vault: "sa.rawkode.academy"
	item: "aws"
	section: "api-tokens"
	field: "access-key"
} @capability("aws")
AWS_SECRET_KEY: #OnePasswordRef & {
	vault: "sa.rawkode.academy"
	item: "aws"
	section: "api-tokens"
	field: "secret-key"
} @capability("aws")

environment: production: {
	DATABASE_URL: "postgresql://production:5432/\(DATABASE_NAME)"
}
