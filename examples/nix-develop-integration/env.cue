package env

// Source nix develop environment
hooks: {
	onEnter: {
		command: "nix"
		args: ["develop", "--print-dev-env"]
		source: true
	}
}

env: {
	// Application-specific variables
	APP_NAME:     "my-rust-app"
	RUST_LOG:     "debug"
	DATABASE_URL: "sqlite://./app.db"

	// Override sourced variables if needed
	CARGO_TARGET_DIR: "./target"
}

tasks: {
	"build": {
		description: "Build Rust project with nix toolchain"
		command:     "cargo build --release"
		cache:       true
	}

	"test": {
		description: "Run tests with nix environment"
		command:     "cargo test"
		dependencies: ["build"]
	}

	"dev": {
		description: "Start development server"
		command:     "cargo run --bin server"
	}

	"check": {
		description: "Run clippy and fmt checks"
		command:     "cargo clippy --all-targets --all-features"
	}
}
