package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

env: {
	MONOREPO_ROOT:    "/workspace"
	DEFAULT_REGISTRY: "docker.io"
	CI_ENABLED:       "false"
}
