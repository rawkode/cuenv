package env

import "github.com/rawkode/cuenv"

// Example: Custom transformation and validation resolvers
// Demonstrates advanced patterns for secret processing
env: cuenv.#Env & {
	// Base64 decode a secret
	DECODED_SECRET: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				"echo 'aGVsbG8gd29ybGQ=' | base64 -d"
			]
		}
	}

	// Generate a secure random password
	RANDOM_PASSWORD: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				"openssl rand -base64 32 | tr -d '\\n'"
			]
		}
	}

	// Fetch secret from HTTP API with authentication
	API_SECRET: {
		resolver: {
			command: "curl"
			args: [
				"-s",
				"-H", "Authorization: Bearer $VAULT_TOKEN",
				"https://vault.company.com/v1/secret/data/myapp",
				"|", "jq", "-r", ".data.data.secret"
			]
		}
	}

	// Transform secret with custom validation
	VALIDATED_KEY: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				'''
				key=$(vault kv get -field=api_key secret/myapp)
				if [[ ${#key} -lt 32 ]]; then
					echo "Error: API key too short" >&2
					exit 1
				fi
				echo "$key"
				'''
			]
		}
	}

	// Composite secret from multiple sources
	DATABASE_URL: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				'''
				user=$(vault kv get -field=username secret/db)
				pass=$(vault kv get -field=password secret/db)
				host=$(consul kv get database/host)
				echo "postgres://$user:$pass@$host:5432/myapp"
				'''
			]
		}
	}

	// Time-based secret rotation check
	ROTATED_SECRET: {
		resolver: {
			command: "sh"
			args: [
				"-c",
				'''
				# Check if secret needs rotation (older than 30 days)
				last_updated=$(vault kv metadata get -field=created_time secret/myapp/key)
				current=$(date +%s)
				updated=$(date -d "$last_updated" +%s)
				age_days=$(( (current - updated) / 86400 ))
				
				if [[ $age_days -gt 30 ]]; then
					echo "Warning: Secret is $age_days days old" >&2
				fi
				
				vault kv get -field=value secret/myapp/key
				'''
			]
		}
	}

	// Environment-specific transformations
	environment: {
		development: {
			// Use mock secrets in development
			PAYMENT_KEY: {
				resolver: {
					command: "echo"
					args: ["sk_test_mock_development_key"]
				}
			}
		}
		production: {
			// Use real secrets with validation in production
			PAYMENT_KEY: {
				resolver: {
					command: "sh"
					args: [
						"-c",
						'''
						key=$(vault kv get -field=live_key secret/payments/stripe)
						if [[ ! "$key" =~ ^sk_live_ ]]; then
							echo "Error: Invalid production key format" >&2
							exit 1
						fi
						echo "$key"
						'''
					]
				}
			}
		}
	}

	// Regular environment variables
	APP_NAME: "custom-transform-example"
	ENVIRONMENT: "development"
}