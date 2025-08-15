package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

// Test environment sourcing with a simple shell script
hooks: {
	onEnter: {
		command: "bash"
		args: ["-c", "echo 'export TEST_VAR=\"sourced_value\"'; echo 'export ANOTHER_VAR=\"hello_world\"'"]
		source: true
	}
}

env: {
	// CUE-defined variables
	APP_ENV: "test"
	// TEST_VAR will come from the sourced environment
	// ANOTHER_VAR will also come from the sourced environment
}

tasks: {
	"show-env": {
		description: "Show all environment variables to verify sourcing works"
		command:     "env | grep -E '(TEST_VAR|ANOTHER_VAR|APP_ENV)' | sort"
	}
}
