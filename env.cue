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
		description: "Code formatting tasks"
		mode: "sequential"  // Check first, then optionally apply
		check: {
			description: "Check all code formatting without making changes"
			command:     "treefmt"
			args: ["--fail-on-change"]

			inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
		}
		apply: {
			description: "Apply code formatting changes"
			command:     "treefmt"
			inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
		}
	}

	build: {
		description: "Build commands"
		debug: {
			description: "Build in debug mode"
			command:     "cargo"
			args: ["build"]
			inputs: ["src/**/*.rs", "Cargo.toml", "Cargo.lock", "libcue-bridge/**/*.go"]
		}
		release: {
			description: "Build in release mode"
			command:     "cargo"
			args: ["build", "--release"]
			inputs: ["src/**/*.rs", "Cargo.toml", "Cargo.lock", "libcue-bridge/**/*.go"]
		}
	}

	test: {
		description: "Testing commands"
		mode: "parallel"  // Run tests in parallel for speed
		unit: {
			description: "Run unit tests"
			command:     "cargo"
			args: ["nextest", "run"]
			inputs: ["src/**/*.rs", "tests/**/*.rs", "Cargo.toml"]
		}
		ci: {
			description: "Run tests with CI profile"
			command:     "cargo"
			args: ["nextest", "run", "--profile", "ci"]
			inputs: ["src/**/*.rs", "tests/**/*.rs", "Cargo.toml"]
		}
		coverage: {
			description: "Generate test coverage report"
			command:     "cargo"
			args: ["llvm-cov", "nextest", "--lcov", "--output-path", "lcov.info"]
			inputs: ["src/**/*.rs", "tests/**/*.rs", "Cargo.toml"]
		}
		examples: {
			description: "Test example configurations"
			command:     "scripts/test-examples.sh"
			inputs: ["examples/**/*", "src/**/*.rs"]
		}
	}

	lint: {
		description: "Linting and code quality checks"
		mode: "sequential"  // Fix first, then check
		fix: {
			description: "Auto-fix linting issues"
			command:     "cargo"
			args: ["clippy", "--fix", "--all-targets", "--all-features", "--allow-dirty"]
			inputs: ["src/**/*.rs", "Cargo.toml"]
		}
		check: {
			description: "Check for linting issues (errors as warnings)"
			command:     "cargo"
			args: ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]
			inputs: ["src/**/*.rs", "Cargo.toml"]
		}
	}

	check: {
		description: "Various checks"
		mode: "parallel"  // Run all checks simultaneously
		nix: {
			description: "Check Nix flake"
			command:     "nix"
			args: ["flake", "check"]
			inputs: ["flake.nix", "flake.lock"]
		}
		all: {
			description: "Run all pre-commit checks"
			command:     "bash"
			args: ["-c", "treefmt && cargo clippy --fix --all-targets --all-features --allow-dirty && cargo clippy --all-targets --all-features -- -D warnings && nix flake check"]
			inputs: ["src/**/*", "*.nix", "Cargo.toml"]
		}
	}

	run: {
		description: "Run the application"
		command:     "cargo"
		args: ["run", "--"]
	}

	deps: {
		description: "Dependency management"
		mode: "parallel"  // Update all dependency types at once
		update: {
			description: "Update Rust dependencies"
			command:     "cargo"
			args: ["update"]
		}
		vendor: {
			description: "Vendor Go dependencies"
			command:     "bash"
			args: ["-c", "cd libcue-bridge && go mod vendor"]
		}
		nix: {
			description: "Update Nix flake"
			command:     "nix"
			args: ["flake", "update"]
		}
	}

	sayHello: {
		description: "Prints a greeting message"
		command:     "echo"
		args: ["Hello, world!"]
	}

	// CI workflow demonstrating nested groups
	ci: {
		description: "Complete CI workflow"
		mode: "workflow"  // DAG-based execution
		
		quality: {
			description: "Run all quality checks"
			mode: "parallel"  // Run these in parallel
			
			format: {
				command: "treefmt"
				args: ["--fail-on-change"]
				inputs: ["**/*.rs", "**/*.go", "**/*.cue"]
			}
			lint: {
				command: "cargo"
				args: ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]
				inputs: ["src/**/*.rs"]
			}
			audit: {
				command: "cargo"
				args: ["audit"]
				inputs: ["Cargo.lock"]
			}
		}
		
		test: {
			description: "Run all tests"
			mode: "parallel"
			dependencies: ["quality"]  // Wait for quality checks
			
			unit: {
				command: "cargo"
				args: ["nextest", "run"]
				inputs: ["src/**/*.rs", "tests/**/*.rs"]
			}
			integration: {
				command: "cargo"
				args: ["test", "--test", "*"]
				inputs: ["tests/**/*.rs"]
			}
			examples: {
				command: "scripts/test-examples.sh"
				inputs: ["examples/**/*"]
			}
		}
		
		build: {
			description: "Build release artifacts"
			dependencies: ["test"]
			command: "cargo"
			args: ["build", "--release"]
			outputs: ["target/release/cuenv"]
		}
	}

	// Release workflow
	release: {
		description: "Release process"
		mode: "sequential"  // Must happen in order
		
		version: {
			description: "Update version"
			command: "cargo"
			args: ["update", "-p", "cuenv"]
		}
		commit: {
			description: "Commit changes"
			command: "git"
			args: ["commit", "-am", "release: new version"]
		}
		tag: {
			description: "Create git tag"
			command: "git"
			args: ["tag", "-a", "v$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name == \"cuenv\") | .version')", "-m", "Release"]
		}
		push: {
			description: "Push to origin"
			command: "git"
			args: ["push", "origin", "main", "--tags"]
		}
	}
}
