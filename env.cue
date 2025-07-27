package env

env: {
	// Environment variables for cuenv development
	CARGO_TERM_COLOR: "always"
	RUST_BACKTRACE:   "1"
	CUENV_ENV:        "development"
}

// Task definitions
tasks: {
	"build": {
		description: "Build cuenv in release mode"
		command:     "cargo build --release"
	}

	"build:debug": {
		description: "Build cuenv in debug mode"
		command:     "cargo build"
	}

	"test": {
		description: "Run all tests"
		command:     "cargo test"
	}

	"test:integration": {
		description: "Run integration tests"
		command:     "cargo test --test '*'"
	}

	"test:examples": {
		description: "Test example configurations"
		script: """
			if [ -f "./scripts/test-examples.sh" ]; then
				./scripts/test-examples.sh
			else
				echo "test-examples.sh script not found"
			fi
			"""
	}

	"fmt": {
		description: "Format code"
		command:     "cargo fmt"
	}

	"fmt:check": {
		description: "Check code formatting"
		command:     "cargo fmt --check"
	}

	"clippy": {
		description: "Run clippy lints"
		command:     "cargo clippy -- -D warnings"
	}

	"dev": {
		description: "Run development server with auto-reload"
		command:     "cargo watch -x run"
	}

	"clean": {
		description: "Clean build artifacts"
		command:     "cargo clean"
	}
}
