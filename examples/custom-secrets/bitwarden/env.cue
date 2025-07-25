package env

import "github.com/rawkode/cuenv"

// Example: Bitwarden CLI integration using reusable BitwardenRef
// Requires: bw CLI installed and authenticated
env: cuenv.#Env & {
	// Get password from Bitwarden item
	DATABASE_PASSWORD: cuenv.#BitwardenRef & {
		itemId: "MyApp Database"
		field:  "password"
	}

	// Get custom field from Bitwarden item
	API_KEY: cuenv.#BitwardenRef & {
		itemId: "Stripe API"
		field:  "password"
	}

	// Extract specific field using item name
	OAUTH_CLIENT_SECRET: cuenv.#BitwardenItemRef & {
		name:  "OAuth Client"
		field: "client_secret"
	}

	// Get username from Bitwarden
	SMTP_USERNAME: cuenv.#BitwardenRef & {
		itemId: "Email SMTP"
		field:  "username"
	}

	// Get TOTP code from Bitwarden (inline example for special use case)
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
			DATABASE_PASSWORD: cuenv.#BitwardenItemRef & {
				name:  "Dev Database"
				field: "password"
			}
		}
		production: {
			BW_ORGANIZATION_ID: "prod-org-id"
			DATABASE_PASSWORD: cuenv.#BitwardenItemRef & {
				name:  "Production Database"
				field: "password"
			}
		}
	}

	// Regular environment variables
	APP_NAME: "bitwarden-example"
	BW_SESSION: "$BW_SESSION"
}