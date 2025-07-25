package env

// Hook definitions with constraints
hooks: {
	// Hook that only runs if devenv is installed
	onEnter: {
		command: "devenv"
		args: ["up"]
		constraints: [
			{
				commandExists: {
					command: "devenv"
				}
			},
			{
				fileExists: {
					path: "devenv.nix"
				}
			}
		]
	}

	// Hook that runs cleanup only if needed
	onExit: {
		command: "echo"
		args: ["🧹 Cleaning up development environment..."]
		constraints: [
			{
				envVarEquals: {
					var: "CLEANUP_MODE"
					value: "auto"
				}
			}
		]
	}
}

env: {
	// Development environment variables
	DATABASE_URL: "postgres://localhost/myapp_dev"
	API_PORT:     "3000"
	DEBUG_MODE:   "true"
	CLEANUP_MODE: "auto"

	// Development root directory
	DEV_ROOT: "$PWD"

	environment: {
		production: {
			DATABASE_URL: "postgres://prod-db/myapp"
			API_PORT:     "8080" 
			DEBUG_MODE:   "false"
			CLEANUP_MODE: "manual"
		}
	}
}