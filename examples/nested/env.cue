package env

import "github.com/rawkode/cuenv"

// Root-level configuration
env: cuenv.#Env & {
	// Root-level environment variables
	APP_NAME:  "MyApp"
	LOG_LEVEL: "info"

	// Root-level computed values
	APP_VERSION: "1.0.0"
	APP_ENV:     "development"

	// Commands available at root
	Commands: {
		test: {
			capabilities: []
		}
		build: {
			capabilities: ["docker"]
		}
	}
}
