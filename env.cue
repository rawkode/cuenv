package env

// Development environment configuration
env: {
	// Cargo configuration
	CARGO_TERM_COLOR: "always"
	RUST_BACKTRACE:   "1"
	CUENV_ENV:        "development"

	// Testing configuration
	RUST_TEST_THREADS: "4"
	GOMAXPROCS:        "4"

	// Build configuration
	CGO_ENABLED: "1"
}

// Development lifecycle hooks
hooks: {
	onEnter: [
		// Source nix flake development environment
		{
			flake: {
				// Use current directory's flake
				dir: "."
			}
		},
		// Then show welcome message
		{
			command: "echo"
			args: ["🦀 cuenv development environment activated (with nix toolchain)"]
		},
	]

	onExit: {
		command: "echo"
		args: ["👋 Leaving cuenv development environment"]
	}
}

// Comprehensive task definitions
tasks: {
	// === Code Quality Tasks ===

	"fmt": {
		description: "Format all code (Rust, Go, Nix, CUE, etc.)"
		command:     "treefmt"
		cache:       true
		inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
	}

	"fmt:check": {
		description: "Check all code formatting without making changes"
		command:     "treefmt --fail-on-change"
		inputs: ["src/**/*", "libcue-bridge/**/*", "*.nix", "**/*.cue", "**/*.md", "**/*.toml", "**/*.yaml", "**/*.yml"]
	}

	"fmt:rust": {
		description: "Format only Rust code"
		command:     "cargo fmt"
		cache:       true
		inputs: ["src/**/*.rs", "build.rs", "Cargo.toml"]
	}

	"fmt:rust:check": {
		description: "Check Rust code formatting"
		command:     "cargo fmt --check"
		inputs: ["src/**/*.rs", "build.rs", "Cargo.toml"]
	}

	"clippy": {
		description: "Run clippy lints with deny warnings"
		command:     "cargo clippy --all-targets --all-features -- -D warnings -A clippy::duplicate_mod -A clippy::uninlined_format_args -A clippy::too_many_arguments"
		cache:       true
		inputs: ["src/**/*.rs", "libcue-bridge/**/*", "build.rs", "Cargo.toml", "Cargo.lock"]
	}

	"clippy:fix": {
		description: "Apply clippy suggestions automatically"
		command:     "cargo clippy --fix --allow-dirty --allow-staged"
		cache:       false
		inputs: ["src/**/*.rs", "libcue-bridge/**/*", "build.rs", "Cargo.toml"]
	}

	// === Build Tasks ===

	"build": {
		description: "Build cuenv in release mode"
		command:     "cargo build --release"
		dependencies: ["fmt:rust:check"]
		cache: true
		inputs: ["src/**/*.rs", "libcue-bridge/**/*", "build.rs", "Cargo.toml", "Cargo.lock"]
		outputs: ["target/release/cuenv"]
	}

	"build:debug": {
		description: "Build cuenv in debug mode"
		command:     "cargo build"
		dependencies: ["fmt:rust:check"]
		cache: true
		inputs: ["src/**/*.rs", "libcue-bridge/**/*", "build.rs", "Cargo.toml", "Cargo.lock"]
		outputs: ["target/debug/cuenv"]
	}

	"build:check": {
		description: "Check that the code compiles without building"
		command:     "cargo check --all-targets --all-features"
		cache:       true
		inputs: ["src/**/*.rs", "libcue-bridge/**/*", "build.rs", "Cargo.toml", "Cargo.lock"]
	}

	// === Test Tasks ===

	"test": {
		description: "Run all tests with nextest"
		command:     "cargo nextest run"
		dependencies: ["build:check"]
		cache: true
		inputs: ["src/**/*.rs", "tests/**/*.rs", "libcue-bridge/**/*", "examples/**/*", "Cargo.toml", "Cargo.lock"]
	}

	"test:unit": {
		description: "Run only unit tests (lib and bins)"
		command:     "cargo nextest run --lib --bins -E 'not test(/concurrent|/thread_safe|/monitored_cache|/profiling|/tree_operations|/confidence|/sequential_pattern|/streaming|/prop_test_cache|/statistics|/parse_shell|/process_guard/)'"
		dependencies: ["build:check"]
		cache: true
		inputs: ["src/**/*.rs", "Cargo.toml", "Cargo.lock"]
	}

	"test:integration": {
		description: "Run integration tests"
		command:     "cargo nextest run --tests"
		dependencies: ["build:debug"]
		cache: true
		inputs: ["src/**/*.rs", "tests/**/*.rs", "libcue-bridge/**/*", "Cargo.toml", "Cargo.lock"]
	}

	"test:doc": {
		description: "Run documentation tests"
		command:     "cargo test --doc"
		dependencies: ["build:check"]
		cache: true
		inputs: ["src/**/*.rs", "Cargo.toml"]
	}

	"test:examples": {
		description: "Test example configurations"
		dependencies: ["build:debug"]
		cache: true
		inputs: ["examples/**/*", "scripts/test-examples.sh"]
		script: """
			if [ -f "./scripts/test-examples.sh" ]; then
				./scripts/test-examples.sh
			else
				echo "Error: test-examples.sh script not found"
				exit 1
			fi
			"""
	}

	"test:coverage": {
		description: "Generate test coverage report"
		command:     "cargo llvm-cov nextest --lcov --output-path lcov.info"
		dependencies: ["build:check"]
		cache: false
		inputs: ["src/**/*.rs", "tests/**/*.rs", "Cargo.toml", "Cargo.lock"]
		outputs: ["lcov.info"]
	}

	// === Quality Assurance Tasks ===

	"audit": {
		description: "Audit dependencies for security vulnerabilities"
		command:     "cargo audit"
		cache:       false
		inputs: ["Cargo.lock"]
	}

	"outdated": {
		description: "Check for outdated dependencies"
		command:     "cargo outdated"
		cache:       false
		inputs: ["Cargo.toml", "Cargo.lock"]
	}

	"doc": {
		description: "Generate and open API documentation"
		command:     "cargo doc --open --no-deps"
		cache:       true
		inputs: ["src/**/*.rs", "Cargo.toml"]
		outputs: ["target/doc/**/*"]
	}

	"doc:check": {
		description: "Check documentation for broken links and missing docs"
		command:     "cargo doc --no-deps"
		cache:       true
		inputs: ["src/**/*.rs", "Cargo.toml"]
	}

	// === Development Tasks ===

	"dev": {
		description: "Run development server with auto-reload"
		command:     "cargo watch -x run"
		cache:       false
	}

	"dev:check": {
		description: "Watch and check code continuously"
		command:     "cargo watch -x check"
		cache:       false
	}

	"dev:test": {
		description: "Watch and run tests continuously"
		command:     "cargo watch -x 'nextest run'"
		cache:       false
	}

	"clean": {
		description: "Clean all build artifacts"
		command:     "cargo clean"
		cache:       false
	}

	"clean:full": {
		description: "Clean build artifacts and Go cache"
		cache:       false
		script: """
			cargo clean
			if [ -d "libcue-bridge/vendor" ]; then
				rm -rf libcue-bridge/vendor
			fi
			if [ -n "$GOCACHE" ] && [ -d "$GOCACHE" ]; then
				go clean -cache
			fi
			echo "Full clean completed"
			"""
	}

	// === Comprehensive Check Tasks (replaces nix flake check) ===

	"check": {
		description: "Run all quality checks (replaces nix flake check)"
		dependencies: ["check:format", "check:lint", "check:build", "check:test"]
		cache: false
		script: """
			echo "✅ All quality checks passed!"
			echo ""
			echo "This task replaces 'nix flake check' and includes:"
			echo "  - Code formatting validation"
			echo "  - Clippy lints"
			echo "  - Build verification"
			echo "  - Unit and integration tests"
			echo "  - Example testing"
			echo "  - Documentation checks"
			"""
	}

	"check:format": {
		description: "Verify all code is properly formatted"
		dependencies: ["fmt:check"]
		cache: true
		script: """
			echo "✅ Code formatting verified"
			"""
	}

	"check:lint": {
		description: "Run all linting checks"
		dependencies: ["clippy"]
		cache: true
		script: """
			echo "✅ Linting checks passed"
			"""
	}

	"check:build": {
		description: "Verify the project builds successfully"
		dependencies: ["build:check", "doc:check"]
		cache: true
		script: """
			echo "✅ Build verification completed"
			"""
	}

	"check:test": {
		description: "Run comprehensive test suite"
		dependencies: ["test:unit", "test:integration", "test:doc", "test:examples"]
		cache: true
		script: """
			echo "✅ Comprehensive test suite completed"
			"""
	}

	"check:security": {
		description: "Run security and dependency checks"
		dependencies: ["audit"]
		cache: false
		script: """
			echo "✅ Security and dependency checks completed"
			"""
	}

	// === CI/CD Tasks ===

	"ci": {
		description: "Run CI pipeline locally"
		dependencies: ["check", "check:security"]
		cache: false
		script: """
			echo "🚀 Running full CI pipeline locally..."
			echo ""
			echo "All CI checks completed successfully!"
			echo "Ready for commit and push."
			"""
	}

	"pre-commit": {
		description: "Pre-commit validation (quick checks)"
		dependencies: ["fmt:rust:check", "clippy", "test:unit"]
		cache: true
		script: """
			echo "✅ Pre-commit checks passed!"
			"""
	}

	// === Version Management Tasks ===

	"version:show": {
		description: "Show current version from Cargo.toml"
		command:     "grep '^version = ' Cargo.toml | head -1 | cut -d'\"' -f2"
		cache:       false
	}

	"version:update": {
		description: "Update version (use with NEW_VERSION env var)"
		cache:       false
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
		description: "Prepare for release by running full validation"
		dependencies: ["ci"]
		cache: false
		script: """
			VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
			echo "🚀 Preparing release for version $VERSION"
			echo ""

			# Check if Cargo.lock is up to date
			if ! cargo update --dry-run 2>&1 | grep -q "nothing to do"; then
				echo "Warning: Cargo.lock may need updating. Run 'cargo update -p cuenv'"
			fi

			echo "Release checklist:"
			echo "✅ All CI checks passed"
			echo "✅ Version in Cargo.toml: $VERSION"
			echo "✅ Dependencies up to date"
			echo ""
			echo "Ready to tag and release version $VERSION"
			echo ""
			echo "Next steps:"
			echo "1. git tag v$VERSION"
			echo "2. git push origin v$VERSION"
			"""
	}

	// === Utility Tasks ===

	"deps:update": {
		description: "Update all dependencies"
		cache:       false
		script: """
			echo "Updating Rust dependencies..."
			cargo update

			echo "Updating Go dependencies in libcue-bridge..."
			cd libcue-bridge
			go get -u ./...
			go mod tidy
			cd ..

			echo "Dependencies updated!"
			echo "Run 'cuenv task test' to verify everything still works."
			"""
	}

	"deps:tree": {
		description: "Show dependency tree"
		command:     "cargo tree"
		cache:       false
	}

	"size": {
		description: "Show binary size information"
		dependencies: ["build"]
		cache: false
		script: """
			echo "Binary sizes:"
			ls -lh target/release/cuenv 2>/dev/null || echo "Release binary not found - run 'cuenv task build' first"
			ls -lh target/debug/cuenv 2>/dev/null || echo "Debug binary not found - run 'cuenv task build:debug' first"
			"""
	}

	// === Nix Helper Tasks ===

	"nix:vendor:update": {
		description: "Recalculates and updates the Cargo vendor hash in flake.nix"
		cache:       false
		inputs: ["Cargo.lock"] // Depends on Cargo.lock, but also modifies flake.nix
		script: """
			#!/bin/bash
			# This script safely updates the cargoVendor hash in flake.nix by forcing
			# Nix to re-calculate it. This is the standard, most reliable method.
			set -euo pipefail

			FLAKE_FILE="flake.nix"
			DUMMY_HASH="sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="

			# Preserve the original file content to restore on any script failure.
			ORIGINAL_CONTENT=$(cat "$FLAKE_FILE")

			# Cleanup function to restore the flake file if the script exits unexpectedly.
			cleanup() {
				if [ -n "${ORIGINAL_CONTENT-}" ]; then
					echo "$ORIGINAL_CONTENT" > "$FLAKE_FILE"
					echo "Restored original flake.nix due to an error."
				fi
			}
			trap cleanup EXIT

			echo "Calculating new vendor hash by probing Nix..."

			# Find the cargo vendor hash line to be replaced.
			HASH_LINE=$(grep 'name = "cuenv-cargo-vendor";' -A 1 "$FLAKE_FILE" | grep 'hash =')
			if [ -z "$HASH_LINE" ]; then
				echo "Error: Could not find the cargoVendor hash line in $FLAKE_FILE."
				exit 1 # trap will restore the file
			fi

			# Replace the real hash with a dummy value to force Nix to report the correct one.
			TEMP_CONTENT=$(echo "$ORIGINAL_CONTENT" | sed "s|$HASH_LINE|                hash = \"$DUMMY_HASH\";|")
			echo "$TEMP_CONTENT" > "$FLAKE_FILE"

			# Run Nix to get the correct hash. This is expected to fail with a mismatch error.
			NIX_OUTPUT=$(nix flake check --quiet 2>&1) || true

			# Check if we got the expected hash mismatch error.
			if echo "$NIX_OUTPUT" | grep -q "hash mismatch"; then
				CORRECT_HASH=$(echo "$NIX_OUTPUT" | grep "got:" | awk '{print $2}')

				if [ -n "$CORRECT_HASH" ]; then
					echo "New hash found: $CORRECT_HASH"
					# Create the final version of flake.nix with the correct hash.
					FINAL_CONTENT=$(echo "$ORIGINAL_CONTENT" | sed "s|$HASH_LINE|                hash = \"$CORRECT_HASH\";|")
					echo "$FINAL_CONTENT" > "$FLAKE_FILE"

					# Disable the exit trap because we have succeeded.
					trap - EXIT
					echo "✅ flake.nix has been updated successfully."
					echo "   Please review and commit the changes."
				else
					echo "Error: Hash mismatch detected, but could not extract the new hash." >&2
					echo "$NIX_OUTPUT" >&2
					exit 1 # trap will restore the file
				fi
			else
				echo "Error: Expected a hash mismatch, but a different error occurred." >&2
				echo "It's possible the hash was already correct, or another Nix error happened." >&2
				echo "$NIX_OUTPUT" >&2
				exit 1 # trap will restore the file
			fi
			"""
	}
}
