package env

import "github.com/rawkode/cuenv"

// Example: Unix pass (password store) integration
// Requires: pass CLI installed and initialized
// Usage: pass init your-gpg-id
env: cuenv.#Env & {
	// Get password from pass store
	DATABASE_PASSWORD: {
		resolver: {
			command: "pass"
			args: ["myapp/database/password"]
		}
	}

	// API key from pass with specific path
	STRIPE_API_KEY: {
		resolver: {
			command: "pass"
			args: ["external/stripe/live-api-key"]
		}
	}

	// Multi-line secret (like SSH private key)
	SSH_PRIVATE_KEY: {
		resolver: {
			command: "pass"
			args: ["infrastructure/ssh/deploy-key"]
		}
	}

	// Get specific line from pass entry (e.g., username and password)
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
			DATABASE_PASSWORD: {
				resolver: {
					command: "pass"
					args: ["myapp/dev/database"]
				}
			}
		}
		staging: {
			DATABASE_PASSWORD: {
				resolver: {
					command: "pass"
					args: ["myapp/staging/database"]
				}
			}
		}
		production: {
			DATABASE_PASSWORD: {
				resolver: {
					command: "pass"
					args: ["myapp/prod/database"]
				}
			}
		}
	}

	// Regular environment variables
	APP_NAME: "pass-example"
	PASSWORD_STORE_DIR: "$HOME/.password-store"
}