package env

import "github.com/rawkode/cuenv"

// Example: Unix pass integration using reusable PassRef
// Requires: pass CLI installed and initialized
env: cuenv.#Env & {
	// Get password from pass store
	DATABASE_PASSWORD: cuenv.#PassRef & {
		path: "myapp/database/password"
	}

	// API key from pass with specific path
	STRIPE_API_KEY: cuenv.#PassRef & {
		path: "external/stripe/live-api-key"
	}

	// Multi-line secret (like SSH private key)
	SSH_PRIVATE_KEY: cuenv.#PassRef & {
		path: "infrastructure/ssh/deploy-key"
	}

	// Custom field extraction for structured pass entries
	SMTP_USERNAME: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				"pass email/smtp | head -n 1"
			]
		}
	}

	SMTP_PASSWORD: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				"pass email/smtp | tail -n +2 | head -n 1"
			]
		}
	}

	// Environment-specific secrets using pass directory structure
	environment: {
		development: {
			DATABASE_PASSWORD: cuenv.#PassRef & {
				path: "myapp/dev/database"
			}
		}
		staging: {
			DATABASE_PASSWORD: cuenv.#PassRef & {
				path: "myapp/staging/database"
			}
		}
		production: {
			DATABASE_PASSWORD: cuenv.#PassRef & {
				path: "myapp/prod/database"
			}
		}
	}

	// Regular environment variables
	APP_NAME: "pass-example"
	PASSWORD_STORE_DIR: "$HOME/.password-store"
}