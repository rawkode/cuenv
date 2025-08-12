package cuenv

env: {
	// Basic environment variables
	PROJECT_NAME: "hooks-preload-example"
	NODE_ENV:     "development"
}

hooks: onEnter: [
	// Regular hook - blocks shell on cd (executes synchronously)
	{
		command: "echo"
		args: ["Setting up environment for hooks-preload example"]
	},

	// Preload hook - runs in background, doesn't block shell
	// User can run ls, cat, etc. immediately
	// But cuenv exec/task will wait for completion
	{
		command: "echo"
		args: ["Starting slow preload task (simulated with sleep)..."]
		preload: true
	},
	{
		command: "sleep"
		args: ["5"] // Simulates a slow operation like nix develop
		preload: true
	},
	{
		command: "echo"
		args: ["Preload task completed!"]
		preload: true
	},

	// Source hook - always runs synchronously to capture environment
	{
		command: "echo"
		args: ["export SOURCED_VAR=from-hook"]
		source: true
	},
]

tasks: {
	"test": {
		command: "echo"
		args: ["Running test task - preload hooks should be complete"]
		description: "Test task that runs after preload hooks"
	}
	"check-env": {
		command: "sh"
		args: ["-c", "echo PROJECT_NAME=$PROJECT_NAME SOURCED_VAR=$SOURCED_VAR"]
		description: "Check environment variables"
	}
}