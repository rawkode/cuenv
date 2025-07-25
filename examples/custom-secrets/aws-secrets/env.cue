package env

import "github.com/rawkode/cuenv"

// Example: AWS Secrets Manager integration using reusable AWSSecretsRef
// Requires: aws CLI installed and configured
env: cuenv.#Env & {
	// Database password from AWS Secrets Manager
	DATABASE_PASSWORD: cuenv.#AWSSecretsRef & {
		secretId: "myapp/database/password"
	}

	// API key with custom region
	STRIPE_API_KEY: cuenv.#AWSSecretsRef & {
		secretId: "external/stripe/api-key"
		region:   "us-west-2"
	}

	// JSON secret with key extraction
	OAUTH_CLIENT_SECRET: cuenv.#AWSSecretsJSONRef & {
		secretId: "myapp/oauth"
		jsonKey:  "client_secret"
	}

	// Cross-account secret access
	CROSS_ACCOUNT_KEY: cuenv.#AWSSecretsRef & {
		secretId: "arn:aws:secretsmanager:us-east-1:123456789012:secret:shared/api-key-AbCdEf"
	}

	// Environment-specific secrets
	environment: {
		development: {
			AWS_REGION: "us-east-1"
			DATABASE_PASSWORD: cuenv.#AWSSecretsRef & {
				secretId: "dev/myapp/database"
			}
		}
		staging: {
			AWS_REGION: "us-east-1"
			DATABASE_PASSWORD: cuenv.#AWSSecretsRef & {
				secretId: "staging/myapp/database"
			}
		}
		production: {
			AWS_REGION: "us-west-2"
			DATABASE_PASSWORD: cuenv.#AWSSecretsRef & {
				secretId: "prod/myapp/database"
				region:   "us-west-2"
			}
		}
	}

	// Regular environment variables
	APP_NAME: "aws-secrets-example"
	DATABASE_URL: "postgres://user:\(DATABASE_PASSWORD)@rds.amazonaws.com/myapp"
}