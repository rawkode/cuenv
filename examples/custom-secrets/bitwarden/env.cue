package env

import "github.com/rawkode/cuenv"

// Example: Bitwarden CLI integration
// Requires: bw CLI installed and authenticated
// Usage: bw login && bw unlock
env: cuenv.#Env & {
	// Get password from Bitwarden item
	DATABASE_PASSWORD: {
		resolver: {
			command: "bw"
			args: [
				"get", "password",
				"MyApp Database"
			]
		}
	}

	// Get custom field from Bitwarden item
	API_KEY: {
		resolver: {
			command: "bw"
			args: [
				"get", "item", "Stripe API",
				"--raw"
			]
		}
	}

	// Extract specific field using jq
	OAUTH_CLIENT_SECRET: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				"bw get item 'OAuth Client' | jq -r .fields[0].value"
			]
		}
	}

	// Get username from Bitwarden
	SMTP_USERNAME: {
		resolver: {
			command: "bw"
			args: [
				"get", "username",
				"Email SMTP"
			]
		}
	}

	// Get TOTP code from Bitwarden
	MFA_TOKEN: {
		resolver: {
			command: "bw"
			args: [
				"get", "totp",
				"GitHub"
			]
		}
	}

	// Environment-specific organization vaults
	environment: {
		development: {
			BW_ORGANIZATION_ID: "dev-org-id"
			DATABASE_PASSWORD: {
				resolver: {
					command: "bw"
					args: [
						"get", "password",
						"Dev Database",
						"--organizationid", "dev-org-id"
					]
				}
			}
		}
		production: {
			BW_ORGANIZATION_ID: "prod-org-id"
			DATABASE_PASSWORD: {
				resolver: {
					command: "bw"
					args: [
						"get", "password",
						"Production Database",
						"--organizationid", "prod-org-id"
					]
				}
			}
		}
	}

	// Regular environment variables
	APP_NAME: "bitwarden-example"
	BW_SESSION: "$BW_SESSION"
}