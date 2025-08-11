package cuenv

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

hooks: onEnter: [
	schema.#NixFlake,
	{command: "echo", args: ["Hello!"]},
]

hooks: onExit: [{command: "echo", args: ["Goodbye!"]}]

env: {
	CARGO_TERM_COLOR:  "always"
	CGO_ENABLED:       "1"
	CUENV_ENV:         "development"
	GOMAXPROCS:        "4"
	RUST_BACKTRACE:    "1"
	RUST_TEST_THREADS: "4"

	environment: production: {
		CUENV_ENV: "production"
	}
}

tasks: {
	fmt: {
		group: "Code formatting tasks"
		check: {
			description: "Check all code formatting without making changes"
			command:     "treefmt"
			args: ["--fail-on-change"]

			inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
		}
	}

	sayHello: {
		description: "Prints a greeting message"
		command:     "echo"
		args: ["Hello, world!"]
	}
}
