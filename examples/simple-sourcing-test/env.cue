package env

import "github.com/rawkode/cuenv"

// Test environment sourcing with a simple shell script
hooks: {
	onEnter: {
		command: "bash"
		args: ["-c", "echo 'export TEST_VAR=\"sourced_value\"'; echo 'export ANOTHER_VAR=\"hello_world\"'"]
		source: true
	}
}

env: cuenv.#Env & {
	// CUE-defined variables (take precedence over sourced ones)
	APP_ENV: "test"
	// This will override any sourced TEST_VAR
	TEST_VAR: "cue_overrides_sourced"
}

tasks: {
	"show-env": {
		description: "Show all environment variables to verify sourcing works"
		command:     "env | grep -E '(TEST_VAR|ANOTHER_VAR|APP_ENV)' | sort"
	}
}
