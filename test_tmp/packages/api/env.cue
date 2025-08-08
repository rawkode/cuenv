package env

env: {API: "true"}
tasks: {
	"build": {
		command:     "echo api > build.txt"
		description: "Build API"
		dependencies: ["packages:web:build"]
	}
}
