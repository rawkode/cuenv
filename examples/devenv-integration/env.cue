package env

// Source devenv environment for seamless nix integration
hooks: {
	onEnter: {
		command: "devenv"
		args: ["print-dev-env"]
		source: true
	}
}

env: {
	// CUE-defined variables (take precedence over sourced ones)
	APP_ENV:      "development"
	DATABASE_URL: "postgres://localhost/myapp_dev"
}

tasks: {
	"dev": {
		description: "Start development server with devenv environment"
		command:     "npm run dev"
	}

	"test": {
		description: "Run tests with devenv tools available"
		command:     "npm test"
	}

	"build": {
		description: "Build with devenv toolchain"
		command:     "npm run build"
		dependencies: ["test"]
	}
}
