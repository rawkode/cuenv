package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

env: {
	NODE_ENV:      "development"
	FRONTEND_PORT: "3000"
	API_URL:       "http://localhost:8080"
	VITE_MODE:     "development"
}

tasks: {
	"build": {
		command: "vite build"
		outputs: ["dist"]
	}
}
