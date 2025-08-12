package examples

env: {
	CI:                       "true"
	GITHUB_ACTIONS:           "true"
	RUNNER_OS:                "Linux"
	DOCKER_BUILDKIT:          "1"
	COMPOSE_DOCKER_CLI_BUILD: "1"
}

tasks: {
	"deploy": {
		command: "deployer"
		dependencies: ["projects:frontend:build"]
		inputs: ["projects:frontend:build#dist"]
	}
}
