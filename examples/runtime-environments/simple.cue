package env

env: {
	NODE_ENV: "development"
	APP_PORT: "3000"
}

// Simple test task
tasks: {
	"test-env": {
		description: "Test environment variables"
		command: "env | grep -E '(NODE_ENV|APP_PORT)'"
	}
	
	"test-simple": {
		description: "Simple test"
		command: "echo 'Hello from cuenv'"
	}
}