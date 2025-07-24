package env

// Root-level environment variables
APP_NAME:     "MyApp"
LOG_LEVEL:    "info"

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
