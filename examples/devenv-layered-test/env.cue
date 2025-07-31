package env

import "github.com/rawkode/cuenv"

// Test devenv integration with layered hooks
hooks: {
	onEnter: [
		// Devenv hook with environment sourcing
		{
			devenv: {
				dir: "."
			}
			source: true
		},
		// Follow-up exec hook that can use devenv environment
		{
			command: "bash"
			args: ["-c", "echo '✨ Devenv loaded! Node version:' $NODE_VERSION"]
		}
	]
}

env: cuenv.#Env & {
	PROJECT_TYPE: "devenv-test"
	BUILD_ENV: "development"
}

tasks: {
	"dev": {
		description: "Start development with devenv environment"
		command: "echo"
		args: ["🚀 Development environment ready with $PROJECT_TYPE"]
	}
	
	"versions": {
		description: "Show tool versions from devenv"
		command: "bash"
		args: ["-c", "echo 'Node:' $NODE_VERSION; echo 'Build env:' $BUILD_ENV"]
	}
}