package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

hooks: onEnter: [
	schema.#Devenv,
	{command: "echo", args: ["Devenv environment loaded!"]},
]

env: {
	PROJECT_NAME: "devenv-native-example"
	NODE_ENV:     "development"
}

tasks: {
	dev: {
		description: "Start development server"
		command:     "echo"
		args: ["Running with Devenv environment"]
	}
}
