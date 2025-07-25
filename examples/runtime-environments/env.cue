package env

env: {
	NODE_ENV: "development"
	APP_PORT: "3000"
}

// Example tasks demonstrating different runtime environments
tasks: {
	// Task running on host (default behavior)
	"host-task": {
		description: "Run task on host system"
		command: "echo 'Running on host: NODE_ENV='$NODE_ENV', APP_PORT='$APP_PORT"
	}

	// Task running in Nix environment
	"nix-task": {
		description: "Run task in Nix environment"
		command: "node --version && npm --version"
		runtime: {
			type: "nix"
			config: {
				shell: "nodejs npm"
				pure: false
			}
		}
	}

	// Task running in Nix flake
	"nix-flake-task": {
		description: "Run task using current directory flake"
		command: "echo 'Running in nix flake environment'"
		runtime: {
			type: "nix"
			config: {
				flake: "."
				pure: false
			}
		}
	}

	// Task running in Docker container
	"docker-task": {
		description: "Run task in Docker container"
		command: "uname -a && echo 'Container working directory:' && pwd && echo 'Environment:' && env | grep -E '(NODE_ENV|APP_PORT)'"
		runtime: {
			type: "docker"
			config: {
				image: "node:18-alpine"
				workDir: "/workspace"
				rm: true
			}
		}
	}

	// Task running in Podman container
	"podman-task": {
		description: "Run task in Podman container"
		command: "echo 'Running in Podman container' && python3 --version"
		runtime: {
			type: "podman"
			config: {
				image: "python:3.11-alpine"
				workDir: "/app"
				rm: true
			}
		}
	}

	// Task running with BuildKit
	"buildkit-task": {
		description: "Run task using BuildKit"
		command: "echo 'Built and running with BuildKit'"
		runtime: {
			type: "buildkit"
			config: {
				image: "alpine:latest"
				context: "."
			}
		}
	}

	// Complex Docker task with volumes and environment
	"docker-complex": {
		description: "Complex Docker task with custom configuration"
		script: """
			echo "Starting complex Docker task..."
			echo "Node version: $(node --version)"
			echo "Working directory: $(pwd)"
			echo "Files in workspace:"
			ls -la
			echo "Environment variables:"
			env | grep -E '(NODE_ENV|APP_PORT|CUSTOM_VAR)'
			"""
		runtime: {
			type: "docker"
			config: {
				image: "node:18"
				workDir: "/workspace"
				env: {
					CUSTOM_VAR: "from-docker-config"
				}
				volumes: [
					"/tmp:/host-tmp:ro"
				]
				rm: true
			}
		}
	}
}