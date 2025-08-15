package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

// Test the new layered hook system
hooks: {
	onEnter: [
		// Simple exec hook
		{
			command: "echo"
			args: ["🚀 Basic exec hook executed!"]
		},
	]

	onExit: [
		{
			command: "echo"
			args: ["👋 Goodbye from exit hook!"]
		},
	]
}

env: {
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
