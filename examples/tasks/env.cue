package examples

capabilities: {
	secrets: {
		commands: ["ls", "terraform"]
	}
}

env: {
	// Environment variables
	DATABASE_URL: "postgres://localhost/myapp"
	API_KEY:      "test-api-key"
	PORT:         "3000"
	SECRET:       "super" @capability("secrets")
}

// Task definitions at top level
tasks: {
	"build": {
		description: "Build the project"
		capabilities: ["secrets"]
		command: "echo 'Building project...' && sleep 1 && echo 'Build complete!'"
		dependencies: ["test", "lint"]
		inputs: ["src/*"]
		outputs: ["build/app"]
	}
	"test": {
		description: "Run tests"
		command:     "echo 'Running tests...' && echo 'Tests passed!'"
		inputs: ["src/*", "tests/*"]
	}
	"lint": {
		description: "Lint the code"
		command:     "ls"
		inputs: ["src/*"]
	}
	"deploy": {
		description: "Deploy the application"
		command:     "echo 'Deploying to production...' && echo 'Using PORT:' $PORT && echo 'Deployment complete!'"
		dependencies: ["build"]
		cache: false
	}
	"clean": {
		description: "Clean build artifacts"
		command:     "echo 'Cleaning build artifacts...'"
		cache:       false
	}
	"script-example": {
		description: "Example using script instead of command"
		cache:       true
		inputs: ["src/*"]
		outputs: ["build/script-output.txt"]
		script: """
			echo "This is a multi-line script"
			echo "Environment variables available:"
			echo "DATABASE_URL: $DATABASE_URL"
			echo "API_KEY: $API_KEY"
			echo "PORT: $PORT"
			mkdir -p build
			echo "Script output" > build/script-output.txt
			"""
	}
	"fail": {
		description: "Task that fails for testing"
		command: "sh -c 'echo \"Starting task...\"; echo \"ERROR: Something went wrong!\" >&2; exit 1'"
	}
}
