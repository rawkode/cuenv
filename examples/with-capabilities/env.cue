package env

// Basic variables (always available)
DATABASE_URL: "postgres://localhost/mydb"
API_KEY:      "test-api-key"
PORT:         "3000"

// AWS capabilities
AWS_REGION:      "us-east-1" @capability("aws")
AWS_ACCESS_KEY:  "aws-access-key-test" @capability("aws")
AWS_SECRET_KEY:  "aws-secret-key-test" @capability("aws")
S3_BUCKET:       "my-app-bucket" @capability("aws")

// Docker capabilities
DOCKER_REGISTRY: "docker.io" @capability("docker")
DOCKER_IMAGE:    "myapp:latest" @capability("docker")

// Commands with capabilities
Commands: {
	deploy: {
		capabilities: ["aws", "docker"]
	}
	test: {
		capabilities: []
	}
	migrate: {
		capabilities: ["database"]
	}
}

// Environment-specific overrides
environment: {
	production: {
		DATABASE_URL: "postgres://prod.example.com/mydb"
		PORT:         "8080"
		AWS_REGION:   "us-west-2" @capability("aws")
	}
	staging: {
		DATABASE_URL: "postgres://staging.example.com/mydb"
		API_KEY:      "staging-api-key"
	}
}
