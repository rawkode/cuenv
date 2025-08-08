package env

tasks: {
	"fmt": {
		description: "Format all code (Rust, Go, Nix, CUE, etc.)"
		command:     "treefmt"
		cache:       true
		inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
	}
}
