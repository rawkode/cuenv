package env

import "github.com/rawkode/cuenv"

// Child configuration
env: cuenv.#Env & {
	// Child overrides - these will override parent values
	DATABASE_URL: "postgres://localhost/child-db"
	API_KEY:      "child-api-key"

	// Additional child-specific variables
	CHILD_SERVICE: "enabled"
	CHILD_PORT:    "4000"

	// Note: In the new structure, composition happens through CUE imports
	// rather than automatic parent directory loading. To reference parent
	// values, you would import the parent package explicitly.

	// Additional commands for child
	capabilities: {
		database: {
			commands: ["psql", "mysql", "migrate"]
		}
		aws: {
			commands: ["aws", "terraform", "pulumi"]
		}
		docker: {
			commands: ["docker", "docker-compose"]
		}
	}
}
