package cuenv

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

hooks: onEnter: [
	schema.#NixFlake & {preload: true},
	{
		// This hook demonstrates background source hooks (minimal delay for development)
		command: "bash"
		args: ["-c", """
			sleep 1
			echo 'export TEST_BG_VAR="background_hook_completed"'
			echo 'export TEST_TIMESTAMP="'$(date +%s)'"'
			"""]
		source: true // Capture the exported environment
	},
]

hooks: onExit: [{command: "echo", args: ["Goodbye!"]}]

env: {
	CARGO_TERM_COLOR:  "always"
	CGO_ENABLED:       "1"
	CUENV_ENV:         "development"
	GOMAXPROCS:        "4"
	RUST_BACKTRACE:    "1"
	RUST_TEST_THREADS: "16"

	environment: production: {
		CUENV_ENV: "production"
	}
}

tasks: {
	count: {
		description: "I can only count to 4"
		// Array structure automatically enables sequential execution in order
		tasks: [
			{
				description: "Count one"
				command: "echo"
				args: ["1"]
			},
			{
				description: "Count two"
				command: "echo"
				args: ["2"]
			},
			{
				description: "Count three"
				command: "echo"
				args: ["3"]
			},
			{
				description: "Count four"
				command: "echo"
				args: ["4"]
			}
		]
	}

	counted: {
		description: "Task that depends on count group completion"
		dependencies: ["count"]
		command: "echo"
		args: ["counted to 4"]
	}

	fmt: {
		description: "Code formatting tasks"
		// Array structure ensures sequential execution - check first, then optionally apply
		tasks: [
			{
				description: "Check all code formatting without making changes"
				command:     "treefmt"
				args: ["--fail-on-change"]
				inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
			},
			{
				description: "Apply code formatting changes"
				command:     "treefmt"
				inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
			}
		]
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
		// Object structure enables parallel execution for speed
		all: {
			description: "Run all tests with optimized parallelism"
			command:     "cargo"
			args: ["nextest", "run", "--test-threads", "16"]
			inputs: ["src/**/*.rs", "tests/**/*.rs", "Cargo.toml"]
		}
		unit: {
			description: "Run unit tests only"
			command:     "cargo"
			args: ["nextest", "run", "--lib"]
			inputs: ["src/**/*.rs", "Cargo.toml"]
		}
		integration: {
			description: "Run integration tests only"
			command:     "cargo"
			args: ["nextest", "run", "--tests"]
			inputs: ["tests/**/*.rs", "Cargo.toml"]
		}
		bdd: {
			description: "Run BDD behavior tests"
			command:     "cargo"
			args: ["test", "--test", "behaviours"]
			inputs: ["tests/behaviours/**/*", "Cargo.toml"]
		}
		shells: {
			description: "Run shell integration tests"
			command:     "cargo"
			args: ["test", "--test", "shells"]
			inputs: ["tests/shells/**/*.rs", "Cargo.toml"]
		}
		examples: {
			description: "Test example configurations"
			command:     "cargo"
			args: ["test", "--test", "examples"]
			inputs: ["examples/**/*", "tests/examples/**/*.rs"]
		}
		ci: {
			description: "Run tests with CI profile and parallel execution"
			command:     "cargo"
			args: ["nextest", "run", "--profile", "ci", "--test-threads", "4"]
			inputs: ["src/**/*.rs", "tests/**/*.rs", "Cargo.toml"]
		}
		coverage: {
			description: "Generate test coverage report with serial execution for accuracy"
			command:     "cargo"
			args: ["llvm-cov", "nextest", "--lcov", "--output-path", "lcov.info", "--profile", "coverage"]
			inputs: ["src/**/*.rs", "tests/**/*.rs", "Cargo.toml"]
		}
	}

	lint: {
		description: "Linting and code quality checks"
		// Array structure ensures sequential execution - fix first, then check
		tasks: [
			{
				description: "Auto-fix linting issues"
				command:     "cargo"
				args: ["clippy", "--fix", "--all-targets", "--all-features", "--allow-dirty"]
				inputs: ["src/**/*.rs", "Cargo.toml"]
			},
			{
				description: "Check for linting issues (errors as warnings)"
				command:     "cargo"
				args: ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]
				inputs: ["src/**/*.rs", "Cargo.toml"]
			}
		]
	}

	check: {
		description: "Various checks"
		// Object structure enables parallel execution of all checks simultaneously
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
		// Object structure enables parallel execution of all dependency updates at once
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

	simpleTest: {
		description: "Simple test task"
		command: "echo"
		args: ["test"]
	}

	examples: {
		description: "Example configurations management"
		lint: {
			description: "Validate all CUE files in examples directory"
			command:     "find"
			args: ["examples", "-name", "*.cue", "-type", "f", "-exec", "cue", "vet", "-c", "{}", ";"]
			inputs: ["examples/**/*.cue"]
		}
	}

	// CI workflow demonstrating nested groups
	ci: {
		description: "Complete CI workflow"
		// Object structure with dependencies creates a parallel execution graph

		quality: {
			description: "Run all quality checks"
			// Object structure enables parallel execution of quality checks
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
			dependencies: ["quality"] // Wait for quality checks
			// Object structure enables parallel test execution
			unit: {
				command: "cargo"
				args: ["nextest", "run", "--lib", "--test-threads", "16"]
				inputs: ["src/**/*.rs"]
			}
			integration: {
				command: "cargo"
				args: ["nextest", "run", "--tests", "--test-threads", "16"]
				inputs: ["tests/**/*.rs"]
			}
			bdd: {
				command: "cargo"
				args: ["test", "--test", "behaviours"]
				inputs: ["tests/behaviours/**/*"]
			}
			examples: {
				command: "cargo"
				args: ["test", "--test", "examples"]
				inputs: ["examples/**/*", "tests/examples/**/*.rs"]
			}
		}

		build: {
			description: "Build release artifacts"
			dependencies: ["test.unit"]
			command: "cargo"
			args: ["build", "--release"]
			outputs: ["target/release/cuenv"]
		}
	}

	// Release workflow
	release: {
		description: "Release process"
		// Array structure ensures sequential execution - must happen in order
		tasks: [
			{
				description: "Update version"
				command:     "cargo"
				args: ["update", "-p", "cuenv"]
			},
			{
				description: "Commit changes"
				command:     "git"
				args: ["commit", "-am", "release: new version"]
			},
			{
				description: "Create git tag"
				command:     "git"
				args: ["tag", "-a", "v$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name == \"cuenv\") | .version')", "-m", "Release"]
			},
			{
				description: "Push to origin"
				command:     "git"
				args: ["push", "origin", "main", "--tags"]
			}
		]
	}
}

// test change to invalidate cache
