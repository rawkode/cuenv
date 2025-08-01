package env

import "github.com/rawkode/cuenv"

// Test the new layered hook system
hooks: {
	onEnter: [
		// Simple exec hook
		{
			command: "echo"
			args: ["🚀 Basic exec hook executed!"]
		},
		// Nix flake hook with sourcing
		{
			flake: {
				dir: "."
			}
			source: true
		},
	]

	onExit: [
		{
			command: "echo"
			args: ["👋 Goodbye from exit hook!"]
		},
	]
}

env: cuenv.#Env & {
	PROJECT_NAME: "layered-hooks-test"
	ENVIRONMENT:  "development"
}

tasks: {
	"test": {
		description: "Test that hooks and environment work"
		command:     "bash"
		args: ["-c", "echo 'Project: $PROJECT_NAME'; echo 'Environment: $ENVIRONMENT'; env | grep -E '(PROJECT_NAME|ENVIRONMENT)' | head -5"]
	}
}
