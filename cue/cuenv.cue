package cuenv

// #Env defines the structure for environment variable configuration
#Env: {
	// Environment variables - keys must be valid environment variable names
	[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret

	// Environment-specific overrides
	environment?: [string]: {
		[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret
	}

	// Command definitions with capability requirements
	Commands?: [string]: #Command

	// Task definitions
	tasks?: [string]: #Task

	// Hook definitions for lifecycle events
	hooks?: {
		// Hook to run when entering the environment
		onEnter?: #HookConfig

		// Hook to run when exiting the environment
		onExit?: #HookConfig
	}
}

// Tasks should be defined at the top level, not nested within env
// Example: tasks: { "build": #Task, "test": #Task }

// #Secret represents a secret reference that will be resolved at runtime
#Secret: {
	resolver: #Resolver
}

// #Resolver defines how to resolve a secret value
#Resolver: {
	command: string
	args: [...string]
}

// #Command defines a command with its required capabilities
#Command: {
	capabilities?: [...string]
}

// #Capability is used as an attribute to tag environment variables
// Usage: VAR_NAME: "value" @capability("aws")
// This is handled as a CUE attribute, not a field

// #HookConfig defines the configuration for a lifecycle hook
#HookConfig: {
	// Command to execute for the hook
	command: string

	// Arguments to pass to the command
	args: [...string]

	// Optional URL that may be used by the hook
	url?: string
}

// #Hook defines the supported hook types
#Hook: "onEnter" | "onExit"

// #OnEnterHook is a convenience type for onEnter hooks
#OnEnterHook: #HookConfig

// #OnExitHook is a convenience type for onExit hooks
#OnExitHook: #HookConfig

// #Task defines the structure for a task that can be executed by cuenv
#Task: {
	// Human-readable description of the task
	description?: string

	// Shell command to execute (mutually exclusive with script)
	command?: string

	// Embedded shell script to execute (mutually exclusive with command)
	script?: string

	// List of task names that must complete successfully before this task runs
	dependencies?: [...string]

	// Working directory for task execution (defaults to current directory)
	workingDir?: string

	// Shell to use for execution (e.g., "bash", "sh", "zsh")
	shell?: string

	// Input files/patterns (for future implementation)
	inputs?: [...string]

	// Output files/patterns (for future implementation)
	outputs?: [...string]
}
