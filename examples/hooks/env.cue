package env

import "github.com/rawkode/cuenv"

// Environment configuration with hooks
env: cuenv.#Env & {
	// Regular environment variables
	DATABASE_URL: "postgres://localhost/mydb"
	API_KEY: "secret123"
	
	// Hook definitions
	hooks: {
		// Hook that runs when entering the environment
		onEnter: {
			command: "echo"
			args: ["🚀 Environment activated! Database: $DATABASE_URL"]
		}
		
		// Hook that runs when exiting the environment
		onExit: {
			command: "echo"
			args: ["👋 Cleaning up environment..."]
		}
	}
}