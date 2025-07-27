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

	"version:show": {
		description: "Show current version from Cargo.toml"
		command:     "grep '^version = ' Cargo.toml | head -1 | cut -d'\"' -f2"
	}

	"version:update": {
		description: "Update version (use with NEW_VERSION env var)"
		script: """
			if [ -z "$NEW_VERSION" ]; then
				echo "Error: NEW_VERSION environment variable must be set"
				echo "Usage: NEW_VERSION=0.3.2 cuenv task version:update"
				exit 1
			fi
			
			# Validate version format
			if ! [[ "$NEW_VERSION" =~ ^[0-9]+\\.[0-9]+\\.[0-9]+$ ]]; then
				echo "Error: Version must be in format X.Y.Z (e.g., 0.3.2)"
				exit 1
			fi
			
			echo "Updating version to $NEW_VERSION..."
			sed -i "s/^version = \\".*\\"/version = \\"$NEW_VERSION\\"/" Cargo.toml
			cargo update -p cuenv
			
			echo "Version updated to $NEW_VERSION"
			echo ""
			echo "Next steps:"
			echo "1. Review changes: git diff"
			echo "2. Commit: git commit -am 'chore: bump version to $NEW_VERSION'"
			echo "3. Tag and push: git tag $NEW_VERSION && git push origin $NEW_VERSION"
			"""
	}

	"release:prepare": {
		description: "Prepare for release by checking version consistency"
		script: """
			VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
			echo "Current version in Cargo.toml: $VERSION"
			
			# Check if Cargo.lock is up to date
			if ! cargo update --dry-run 2>&1 | grep -q "nothing to do"; then
				echo "Warning: Cargo.lock may need updating. Run 'cargo update -p cuenv'"
			fi
			
			echo ""
			echo "Release checklist:"
			echo "✓ Version in Cargo.toml: $VERSION"
			echo "✓ flake.nix reads version from Cargo.toml"
			echo "✓ GitHub Actions will use version from Cargo.toml"
			echo ""
			echo "Ready to tag and release version $VERSION"
			"""
	}
}
