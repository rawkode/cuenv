package env

import p0 "github.com/rawkode/blah/p0:env"

tasks: {
	"fmt": {
		description: "Format all code (Rust, Go, Nix, CUE, etc.)"
		dependencies: [p0.tasks.fmt]
		command: "treefmt"
		cache:   true
		inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
	}
}
