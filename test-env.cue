package env

// Define secret type with resolver
#SecretRef: {
	resolver: {
		cmd: string
		args: [...string]
	}
	...
}

// Echo-based test secret for demonstration
#TestSecret: #SecretRef & {
	value: string
	
	resolver: {
		cmd: "echo"
		args: [value]
	}
}

// Regular variables
APP_NAME: "test-app"
PORT: 8080

// Test secret that will be resolved via echo command
TEST_SECRET: #TestSecret & {
	value: "super-secret-value"
}

ANOTHER_SECRET: #TestSecret & {
	value: "another-secret-123"
}