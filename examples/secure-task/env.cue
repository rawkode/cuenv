package examples

env: {
	// Environment variables
	DATABASE_URL: "postgres://localhost/myapp"
	API_KEY:      "test-api-key"
	PORT:         "3000"
}

// Task definitions with security restrictions
tasks: {
	"secure-build": {
		description: "Build the project with restricted filesystem access"
		command:     "echo 'Building project securely...' && sleep 1 && echo 'Build complete!'"
		security: {
			restrictDisk: true
			readOnlyPaths: ["/usr", "/lib", "/bin"]
			readWritePaths: ["/tmp", "./build"]
		}
	}
	"network-task": {
		description: "Task that needs network access but with restrictions"
		command:     "echo 'Downloading dependencies...' && curl --version"
		security: {
			restrictNetwork: true
			allowedHosts: ["api.example.com", "registry.npmjs.org"]
		}
	}
	"fully-restricted": {
		description: "Task with both disk and network restrictions"
		command:     "echo 'Running in secure sandbox'"
		security: {
			restrictDisk:    true
			restrictNetwork: true
			readOnlyPaths: ["/usr/bin", "/bin"]
			readWritePaths: ["/tmp"]
			allowedHosts: ["localhost"]
		}
	}
	"unrestricted": {
		description: "Task without security restrictions"
		command:     "echo 'Running without restrictions' && ls -la /"
	}
}
