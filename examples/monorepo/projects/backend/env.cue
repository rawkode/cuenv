package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

env: {
	GO_ENV:       "development"
	BACKEND_PORT: "8080"
	DATABASE_URL: "postgres://localhost/dev"
	REDIS_URL:    "redis://localhost:6379"
	LOG_LEVEL:    "debug"
}
