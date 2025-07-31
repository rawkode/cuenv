package env

import "github.com/rawkode/cuenv"

// Source nix develop environment for seamless integration
hooks: {
	onEnter: {
		command: "nix"
		args: ["develop", "--print-dev-env"]
		source: true
	}
}

env: cuenv.#Env & {
	// CUE-defined variables (take precedence over sourced ones)
	APP_ENV:    "development"
	PROJECT:    "nix-cuenv-integration"
}

tasks: {
	"show-versions": {
		description: "Show tool versions from nix develop environment"
		command:     "bash"
		args: ["-c", "echo 'Node: $NODE_VERSION'; echo 'Python: $PYTHON_VERSION'; echo 'Shell Active: $DEV_SHELL_ACTIVE'; echo 'Project: $PROJECT'"]
	}

	"node-version": {
		description: "Show Node.js version (sourced from nix develop)"
		command:     "node"
		args: ["--version"]
	}
}