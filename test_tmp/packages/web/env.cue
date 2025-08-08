package env

env: {WEB: "true"}
tasks: {
	"build": {
		command:     "echo web > build.txt"
		description: "Build web"
	}
}
