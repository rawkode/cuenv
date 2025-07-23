package cuenv

// Command capability mappings
Commands: {
    terraform: capabilities: ["aws", "cloudflare"]
    aws: capabilities: ["aws"]
    gcloud: capabilities: ["gcp"]
    kubectl: capabilities: ["kubernetes"]
}

// Base environment variables (always loaded)
DATABASE_NAME: "myapp"
DATABASE_URL: "postgresql://localhost:5432/\(DATABASE_NAME)"
API_BASE_URL: "https://api.example.com/v1"
PORT: 8080
LOG_LEVEL: "info"

// AWS-specific variables (only loaded when aws capability is active)
AWS_ACCESS_KEY: "dev-access-key" @capability("aws")
AWS_SECRET_KEY: "dev-secret-key" @capability("aws")
AWS_REGION: "us-east-1" @capability("aws")

// Cloudflare variables
CLOUDFLARE_API_TOKEN: "cf-token-dev" @capability("cloudflare")
CLOUDFLARE_ZONE_ID: "zone-123" @capability("cloudflare")

// GCP variables
GOOGLE_APPLICATION_CREDENTIALS: "/path/to/dev-creds.json" @capability("gcp")
GCP_PROJECT: "dev-project" @capability("gcp")

// Environment-specific overrides
environment: {
    production: {
        DATABASE_URL: "postgresql://prod-db.internal:5432/\(DATABASE_NAME)"
        LOG_LEVEL: "warn"
        AWS_REGION: "us-west-2" @capability("aws")
        AWS_ACCESS_KEY: "op://Production/aws/access-key" @capability("aws")
        AWS_SECRET_KEY: "op://Production/aws/secret-key" @capability("aws")
        CLOUDFLARE_API_TOKEN: "op://Production/cloudflare/api-token" @capability("cloudflare")
        GCP_PROJECT: "prod-project" @capability("gcp")
    }
    
    staging: {
        DATABASE_URL: "postgresql://staging-db.internal:5432/\(DATABASE_NAME)_staging"
        LOG_LEVEL: "debug"
        AWS_ACCESS_KEY: "op://Staging/aws/access-key" @capability("aws")
        AWS_SECRET_KEY: "op://Staging/aws/secret-key" @capability("aws")
    }
}
