package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

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
	"github-api": {
		description: "Can only access GitHub API"
		command:     "curl https://api.github.com/repos/rawkode/cuenv"
		security: {
			restrictNetwork: true
			allowedHosts: ["api.github.com"]
		}
	}
	"npm-install": {
		description: "Can only access npm registry"
		command:     "echo 'npm install express (simulated)' && curl --head https://registry.npmjs.org"
		security: {
			restrictNetwork: true
			allowedHosts: ["*.npmjs.org", "*.npmjs.com"]
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
