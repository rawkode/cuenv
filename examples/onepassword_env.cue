package cuenv

// Example CUE file showing 1Password integration
// This demonstrates how to reference 1Password secrets in your environment

// Define the OnePasswordRef schema locally
#OnePasswordRef: {
	ref: string
	resolver: {
		command: "op"
		args: ["read", ref]
	}
}

// Database configuration with 1Password secret
DB_HOST: "postgres.example.com"
DB_PORT: "5432"
DB_USER: "appuser"
DB_PASSWORD: #OnePasswordRef & {
    ref: "op://korora-tech.cuenv/test-password/password"
}

// API keys from 1Password
STRIPE_API_KEY: #OnePasswordRef & {
    ref: "op://MyVault/Stripe/api_key"
}

GITHUB_TOKEN: #OnePasswordRef & {
    ref: "op://Development/GitHub/personal_access_token"
}

// AWS credentials from 1Password
AWS_ACCESS_KEY_ID: #OnePasswordRef & {
    ref: "op://AWS/Production/access_key_id"
}

AWS_SECRET_ACCESS_KEY: #OnePasswordRef & {
    ref: "op://AWS/Production/secret_access_key"
}

// Regular environment variables mixed with secrets
NODE_ENV: "production"
LOG_LEVEL: "info"
APP_NAME: "my-secure-app"

// Alternative: You can also define the resolver inline without import
// DB_PASSWORD_ALT: {
//     resolver: {
//         command: "op"
//         args: ["read", "op://korora-tech.cuenv/test-password/password"]
//     }
// }
