package cuenv

// #Env defines the structure for environment variable configuration
#Env: {
	// Environment variables - keys must be valid environment variable names
	[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret
	
	// Environment-specific overrides
	environment?: [string]: {
		[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret
	}
	
	// Command definitions with capability requirements
	Commands?: [string]: #Command
}

// #Secret represents a secret reference that will be resolved at runtime
#Secret: {
	resolver: #Resolver
}

// #Resolver defines how to resolve a secret value
#Resolver: {
	command: string
	args: [...string]
}

// #Command defines a command with its required capabilities
#Command: {
	capabilities?: [...string]
}

// #Capability is used as an attribute to tag environment variables
// Usage: VAR_NAME: "value" @capability("aws")
// This is handled as a CUE attribute, not a field