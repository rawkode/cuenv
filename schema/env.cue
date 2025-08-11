package schema

#Environment: {
	[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret
}


// #Env defines the structure for environment variable configuration
#Env: {
	// Environment variables - keys must be valid environment variable names
	[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret

	// Environment-specific overrides
	environment?: [string]: {
		[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret
	}
}
