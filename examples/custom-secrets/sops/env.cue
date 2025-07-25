package env

import "github.com/rawkode/cuenv"

// Example: Mozilla SOPS (Secrets OPerationS) integration
// Requires: sops CLI installed and configured
// Usage: Configure age, GPG, or cloud KMS keys
env: cuenv.#Env & {
	// Decrypt SOPS file and extract specific key
	DATABASE_PASSWORD: {
		resolver: {
			command: "sops"
			args: [
				"--decrypt", "--extract", '["database"]["password"]',
				"secrets.yaml"
			]
		}
	}

	// Extract API key from SOPS JSON file
	API_KEY: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				"sops --decrypt secrets.json | jq -r .api_key"
			]
		}
	}

	// Extract from environment-specific SOPS file
	JWT_SECRET: {
		resolver: {
			command: "sops"
			args: [
				"--decrypt", "--extract", '["jwt_secret"]',
				"secrets/production.yaml"
			]
		}
	}

	// Decrypt entire SOPS file and extract with yq
	SMTP_PASSWORD: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				"sops --decrypt config/secrets.yaml | yq .smtp.password"
			]
		}
	}

	// Environment-specific SOPS files
	environment: {
		development: {
			SOPS_FILE: "secrets/dev.yaml"
			DATABASE_PASSWORD: {
				resolver: {
					command: "sops"
					args: [
						"--decrypt", "--extract", '["database"]["password"]',
						"secrets/dev.yaml"
					]
				}
			}
		}
		staging: {
			SOPS_FILE: "secrets/staging.yaml"
			DATABASE_PASSWORD: {
				resolver: {
					command: "sops"
					args: [
						"--decrypt", "--extract", '["database"]["password"]',
						"secrets/staging.yaml"
					]
				}
			}
		}
		production: {
			SOPS_FILE: "secrets/prod.yaml"
			DATABASE_PASSWORD: {
				resolver: {
					command: "sops"
					args: [
						"--decrypt", "--extract", '["database"]["password"]',
						"secrets/prod.yaml"
					]
				}
			}
		}
	}

	// Regular environment variables
	APP_NAME: "sops-example"
	CONFIG_DIR: "config"
}