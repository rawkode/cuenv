package env

import "github.com/rawkode/cuenv"

// Environment configuration with structured secrets
env: cuenv.#Env & {
	// Regular variables
	DATABASE_URL: "postgres://localhost/mydb"
	PORT:         "3000"

	// Using 1Password secret references
	AWS_ACCESS_KEY_ID:     "op://Personal/aws/key"
	AWS_SECRET_ACCESS_KEY: "op://Personal/aws/secret"
	DATABASE_PASSWORD:     "op://Work/database/password"
	STRIPE_API_KEY:        "op://Work/stripe/api-key"

	// Secret with section reference
	GITHUB_TOKEN: "op://Personal/GitHub/token"

	// Mixing regular and secret values
	API_ENDPOINT: "https://api.example.com"
	API_KEY:      "op://Work/api/key"

	// Environment-specific secrets
	environment: {
		production: {
			DATABASE_PASSWORD: "op://Production/database/password"
			AWS_ACCESS_KEY_ID: "op://Production/aws/access-key"
		}
		staging: {
			DATABASE_PASSWORD: "op://Staging/database/password"
		}
	}
}
