package env

import "github.com/rawkode/cuenv"

// Example: SOPS (Mozilla Secrets OPerationS) integration using reusable SOPSRef
// Requires: sops CLI installed and configured with age/GPG/KMS keys
env: cuenv.#Env & {
	// Decrypt SOPS file and extract specific key
	DATABASE_PASSWORD: cuenv.#SOPSRef & {
		file: "secrets.yaml"
		path: "database.password"
	}

	// Extract API key from SOPS JSON file
	API_KEY: cuenv.#SOPSJSONRef & {
		file:    "secrets.json"
		jsonKey: ".api_key"
	}

	// Extract from environment-specific SOPS file
	JWT_SECRET: cuenv.#SOPSRef & {
		file: "secrets/production.yaml"
		path: "jwt_secret"
	}

	// Extract SMTP password with YAML structure
	SMTP_PASSWORD: cuenv.#SOPSRef & {
		file: "config/secrets.yaml"
		path: "smtp.password"
	}

	// Environment-specific SOPS files
	environment: {
		development: {
			SOPS_FILE: "secrets/dev.yaml"
			DATABASE_PASSWORD: cuenv.#SOPSRef & {
				file: "secrets/dev.yaml"
				path: "database.password"
			}
		}
		staging: {
			SOPS_FILE: "secrets/staging.yaml"
			DATABASE_PASSWORD: cuenv.#SOPSRef & {
				file: "secrets/staging.yaml"
				path: "database.password"
			}
		}
		production: {
			SOPS_FILE: "secrets/prod.yaml"
			DATABASE_PASSWORD: cuenv.#SOPSRef & {
				file: "secrets/prod.yaml"
				path: "database.password"
			}
		}
	}

	// Regular environment variables
	APP_NAME: "sops-example"
	CONFIG_DIR: "config"
}