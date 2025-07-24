package env

import "github.com/rawkode/cuenv"

// Environment configuration with various secret registry types
env: cuenv.#Env & {
	// Regular variables
	DATABASE_URL: "postgres://localhost/mydb"
	PORT:         "3000"

	// GitHub registry secrets
	GITHUB_TOKEN:  "github://myorg/myrepo/GITHUB_TOKEN"
	NPM_TOKEN:     "github://myorg/myrepo/NPM_TOKEN"
	DEPLOY_KEY:    "github://myorg/myrepo/DEPLOY_KEY"

	// GitLab registry secrets
	GITLAB_TOKEN:  "gitlab://mygroup/myproject/GITLAB_TOKEN"
	CI_REGISTRY:   "gitlab://mygroup/myproject/CI_REGISTRY"

	// AWS Secrets Manager
	DB_PASSWORD:   "aws-secret://prod/database/password"
	API_SECRET:    "aws-secret://prod/api/secret"

	// Azure Key Vault
	AZURE_KEY:     "azure-keyvault://myvault/keys/mykey"
	AZURE_SECRET:  "azure-keyvault://myvault/secrets/mysecret"

	// Google Secret Manager
	GCP_KEY:       "gcp-secret://myproject/api-key"
	GCP_SECRET:    "gcp-secret://myproject/db-password"

	// Hashicorp Vault
	VAULT_TOKEN:   "vault://secret/data/myapp/token"
	VAULT_CERT:    "vault://pki/issue/my-role"
}
