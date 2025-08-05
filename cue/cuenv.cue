package cuenv

// #Env defines the structure for environment variable configuration
#Env: {
	// Environment variables - keys must be valid environment variable names
	[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret

	// Environment-specific overrides
	environment?: [string]: {
		[=~"^[A-Z][A-Z0-9_]*$"]: string | #Secret
	}

	// Capability definitions with associated commands
	capabilities?: [string]: #Capability

	// Task definitions
	tasks?: [string]: #Task

	// Hook definitions for lifecycle events
	hooks?: {
		// Hook to run when entering the environment
		onEnter?: #Hook | [...#Hook]

		// Hook to run when exiting the environment
		onExit?: #Hook | [...#Hook]
	}
}

// Tasks should be defined at the top level, not nested within env
// Example: tasks: { "build": #Task, "test": #Task }

// #Secret represents a secret reference that will be resolved at runtime
#Secret: {
	resolver: #Resolver
	...
}

// #Resolver defines how to resolve a secret value
#Resolver: {
	command: string
	args: [...string]
	...
}

// #Capability defines a capability with its associated commands
#Capability: {
	commands?: [...string]
}

// @capability is used as an attribute to tag environment variables
// Usage: VAR_NAME: "value" @capability("aws")
// This is handled as a CUE attribute, not a field

// #Exec defines the base execution primitive
#Exec: {
	// Command to execute
	command: string

	// Arguments to pass to the command
	args?: [...string]

	// Working directory for execution (defaults to current directory)
	dir?: string

	// Input files/patterns for watching (future file watching implementation)
	inputs?: [...string]

	// Source the command output as shell exports (like `source script.sh`)
	// When true, parses stdout for `export VAR=value` statements and adds them to the environment
	source?: bool

	// To be extended
	...
}

// #Hook defines the supported hook types for lifecycle events
#Hook: #Exec | #NixFlake | #Devenv

// #NixFlake defines nix flake integration
#NixFlake: #Exec & {
	flake: {
		// Flake directory (defaults to current directory)
		dir?: string

		// Explicit flake reference (e.g., "github:owner/repo", "path:./other")
		reference?: string

		// DevShell name to use (defaults to "default")
		shell?: string

		// Allow impure evaluation
		impure?: bool
	}
}

// #Devenv defines devenv integration
#Devenv: #Exec & {
	devenv: {
		// Devenv directory (defaults to current directory)
		dir?: string

		// Devenv profile name
		profile?: string

		// Additional devenv options
		options?: [...string]
	}
}

// #Task defines the structure for a task that can be executed by cuenv
#Task: #Exec & {
	// Human-readable description of the task
	description?: string

	// Embedded shell script to execute (mutually exclusive with command)
	script?: string

	// List of task names that must complete successfully before this task runs
	dependencies?: [...string]

	// Shell to use for execution (e.g., "bash", "sh", "zsh")
	shell?: string

	// Output files/patterns (for future implementation)
	outputs?: [...string]

	// Cache configuration for this task
	// Can be a boolean (true/false) or an object with advanced settings
	cache?: bool | #CacheConfig
}

// #CacheConfig defines advanced cache configuration for tasks
#CacheConfig: {
	// Whether caching is enabled for this task (default: true)
	enabled?: bool

	// Custom environment filtering configuration
	env?: #CacheEnvConfig
}

// #CacheEnvConfig defines environment variable filtering for cache keys
#CacheEnvConfig: {
	// Patterns to include (allowlist) - supports wildcards like "BUILD_*"
	include?: [...string]

	// Patterns to exclude (denylist) - supports wildcards like "*_SECRET"
	exclude?: [...string]

	// Whether to use smart defaults for common build tools (default: true)
	useSmartDefaults?: bool
}
