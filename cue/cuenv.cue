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

// #Capability defines a capability with its associated commands
#Capability: {
	commands?: [...string]
}

// @capability is used as an attribute to tag environment variables
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

	// Runtime environment configuration (optional)
	runtime?: #Runtime

	// Input files/patterns (for future implementation)
	inputs?: [...string]

	// Output files/patterns (for future implementation)
	outputs?: [...string]
}

// #Runtime defines the runtime environment for task execution
#Runtime: {
	// Runtime type
	type: #RuntimeType

	// Runtime-specific configuration
	config?: #RuntimeConfig
}

// #RuntimeType defines the supported runtime environments
#RuntimeType: "host" | "nix" | "docker" | "podman" | "buildkit"

// #RuntimeConfig defines runtime-specific configuration
#RuntimeConfig: {
	// Nix runtime configuration
	if type == "nix" {
		// Nix shell expression or flake reference
		shell?: string

		// Nix flake reference (e.g., "github:org/repo" or ".")
		flake?: string

		// Pure nix-shell (no external environment)
		pure?: bool

		// Additional nix arguments
		args?: [...string]
	}

	// Docker runtime configuration
	if type == "docker" {
		// Docker image to use
		image: string

		// Working directory inside container
		workDir?: string

		// Environment variables to pass to container
		env?: [string]: string

		// Volume mounts (host:container format)
		volumes?: [...string]

		// Network mode
		network?: string

		// Additional docker run arguments
		args?: [...string]

		// Remove container after execution
		rm?: bool
	}

	// Podman runtime configuration
	if type == "podman" {
		// Podman image to use
		image: string

		// Working directory inside container
		workDir?: string

		// Environment variables to pass to container
		env?: [string]: string

		// Volume mounts (host:container format)
		volumes?: [...string]

		// Network mode
		network?: string

		// Additional podman run arguments
		args?: [...string]

		// Remove container after execution
		rm?: bool
	}

	// BuildKit LLB runtime configuration
	if type == "buildkit" {
		// Base image for the build context
		image: string

		// Dockerfile content or reference
		dockerfile?: string

		// Build context path
		context?: string

		// Build arguments
		buildArgs?: [string]: string

		// Target stage in multi-stage build
		target?: string

		// Additional buildctl arguments
		args?: [...string]
	}
}
