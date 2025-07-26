package env

env: {
	// Environment variables
	DATABASE_URL: "postgres://localhost/myapp"
	API_KEY:      "test-api-key"
	PORT:         "3000"
}

// Task definitions with security restrictions inferred from inputs/outputs
tasks: {
	"process-data": {
		description: "Process data files with inferred security restrictions"
		command:     "cat input.txt | sort > output.txt && echo 'Processing complete'"
		inputs: [
			"./input.txt",
			"./config.json"
		]
		outputs: [
			"./output.txt",
			"./logs/"
		]
		security: {
			// Automatically infer disk restrictions from inputs/outputs
			inferFromInputsOutputs: true
			// Additional paths can still be specified manually
			readOnlyPaths: ["/usr/bin", "/bin"]
		}
	}
	"build-project": {
		description: "Build project with manual and inferred restrictions"
		command:     "echo 'Building...' && mkdir -p build && echo 'Built' > build/result.txt"
		inputs: [
			"./src/",
			"./package.json"
		]
		outputs: [
			"./build/",
			"./dist/"
		]
		security: {
			inferFromInputsOutputs: true
			restrictNetwork:         true
			allowedHosts: ["registry.npmjs.org", "api.github.com"]
		}
	}
	"network-download": {
		description: "Download with network restrictions but no file inference"
		command:     "curl -s https://api.example.com/data"
		security: {
			restrictNetwork: true
			allowedHosts: ["api.example.com"]
			// No file restrictions inferred since no inputs/outputs specified
		}
	}
	"audit-example": {
		description: "Example task to run with audit mode"
		command:     "ls -la /tmp && curl --version && echo 'Audit this!'"
		// No security section - will show all access in audit mode
	}
}