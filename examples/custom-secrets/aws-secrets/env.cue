package env

import "github.com/rawkode/cuenv"

// Example: AWS Secrets Manager integration
// Requires: aws CLI installed and configured
// Usage: aws configure or use IAM roles/instance profiles
env: cuenv.#Env & {
	// Database password from AWS Secrets Manager
	DATABASE_PASSWORD: {
		resolver: {
			command: "aws"
			args: [
				"secretsmanager", "get-secret-value",
				"--secret-id", "myapp/database/password",
				"--query", "SecretString",
				"--output", "text"
			]
		}
	}

	// API key with custom region
	STRIPE_API_KEY: {
		resolver: {
			command: "aws"
			args: [
				"secretsmanager", "get-secret-value",
				"--region", "us-west-2",
				"--secret-id", "external/stripe/api-key",
				"--query", "SecretString",
				"--output", "text"
			]
		}
	}

	// JSON secret with jq extraction
	OAUTH_CLIENT_SECRET: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				"aws secretsmanager get-secret-value --secret-id myapp/oauth --query SecretString --output text | jq -r .client_secret"
			]
		}
	}

	// Cross-account secret access
	CROSS_ACCOUNT_KEY: {
		resolver: {
			command: "aws"
			args: [
				"secretsmanager", "get-secret-value",
				"--secret-id", "arn:aws:secretsmanager:us-east-1:123456789012:secret:shared/api-key-AbCdEf",
				"--query", "SecretString",
				"--output", "text"
			]
		}
	}

	// Environment-specific secrets
	environment: {
		development: {
			AWS_REGION: "us-east-1"
			DATABASE_PASSWORD: {
				resolver: {
					command: "aws"
					args: [
						"secretsmanager", "get-secret-value",
						"--secret-id", "dev/myapp/database",
						"--query", "SecretString",
						"--output", "text"
					]
				}
			}
		}
		staging: {
			AWS_REGION: "us-east-1"
			DATABASE_PASSWORD: {
				resolver: {
					command: "aws"
					args: [
						"secretsmanager", "get-secret-value",
						"--secret-id", "staging/myapp/database",
						"--query", "SecretString",
						"--output", "text"
					]
				}
			}
		}
		production: {
			AWS_REGION: "us-west-2"
			DATABASE_PASSWORD: {
				resolver: {
					command: "aws"
					args: [
						"secretsmanager", "get-secret-value",
						"--secret-id", "prod/myapp/database",
						"--query", "SecretString",
						"--output", "text"
					]
				}
			}
		}
	}

	// Regular environment variables
	APP_NAME: "aws-secrets-example"
	DATABASE_URL: "postgres://user:\(DATABASE_PASSWORD)@rds.amazonaws.com/myapp"
}