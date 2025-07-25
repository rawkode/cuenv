package env

import "github.com/rawkode/cuenv"

// Environment configuration with capabilities
env: cuenv.#Env & {
	// Basic variables (always available)
	DATABASE_URL: "postgres://localhost/mydb"
	API_KEY:      "test-api-key"
	PORT:         "3000"

	// AWS capabilities
	AWS_REGION:     "us-east-1"           @capability("aws")
	AWS_ACCESS_KEY: "aws-access-key-test" @capability("aws")
	AWS_SECRET_KEY: "aws-secret-key-test" @capability("aws")
	S3_BUCKET:      "my-app-bucket"       @capability("aws")

	// Docker capabilities
	DOCKER_REGISTRY: "docker.io"    @capability("docker")
	DOCKER_IMAGE:    "myapp:latest" @capability("docker")

	// Capabilities with associated commands
	capabilities: {
		aws: {
			commands: ["aws", "pulumi", "terraform"]
		}
		docker: {
			commands: ["docker", "docker-compose"]
		}
		database: {
			commands: ["psql", "mysql", "migrate"]
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
}

// Capabilities can also be defined at the top level
capabilities: env.capabilities
