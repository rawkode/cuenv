package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

env: {
	DATABASE_URL: "postgres://localhost/myapp"
	API_KEY:      "test-api-key"
	PORT:         "3000"
}

// Example of different task group execution modes
tasks: {
	// Workflow mode - DAG with dependencies
	"ci": {
		description: "CI workflow with dependency graph"
		mode: "workflow"

		"lint": {
			description: "Run linters"
			command: "echo 'Running linters...'"
			inputs: ["src/**/*.rs"]
		}
		"test": {
			description: "Run tests"
			command: "echo 'Running tests...'"
			dependencies: ["lint"]
			inputs: ["src/**/*.rs", "tests/**/*.rs"]
		}
		"build": {
			description: "Build the project"
			command: "echo 'Building project...'"
			dependencies: ["test"]
			outputs: ["target/release/app"]
		}
		"package": {
			description: "Package the release"
			command: "echo 'Creating release package...'"
			dependencies: ["build"]
			outputs: ["release.tar.gz"]
		}
	}

	// Sequential mode - tasks run one after another
	"deploy": {
		description: "Deployment process"
		mode: "sequential"

		"backup": {
			description: "Backup current deployment"
			command: "echo 'Creating backup...'"
		}
		"upload": {
			description: "Upload new version"
			command: "echo 'Uploading files...'"
		}
		"migrate": {
			description: "Run database migrations"
			command: "echo 'Running migrations...'"
		}
		"verify": {
			description: "Verify deployment"
			command: "echo 'Verifying deployment...'"
		}
		"notify": {
			description: "Send notifications"
			command: "echo 'Deployment complete!'"
		}
	}

	// Parallel mode - all tasks run simultaneously
	"assets": {
		description: "Build all assets in parallel"
		mode: "parallel"

		"css": {
			description: "Compile CSS"
			command: "echo 'Compiling CSS...'"
			inputs: ["styles/**/*.scss"]
			outputs: ["dist/styles.css"]
		}
		"javascript": {
			description: "Bundle JavaScript"
			command: "echo 'Bundling JavaScript...'"
			inputs: ["src/**/*.js"]
			outputs: ["dist/app.js"]
		}
		"images": {
			description: "Optimize images"
			command: "echo 'Optimizing images...'"
			inputs: ["assets/images/*"]
			outputs: ["dist/images/*"]
		}
		"fonts": {
			description: "Process fonts"
			command: "echo 'Processing fonts...'"
			inputs: ["assets/fonts/*"]
			outputs: ["dist/fonts/*"]
		}
	}

	// Group mode (default) - simple collection
	"utils": {
		description: "Utility tasks"
		// mode: "group" is default, no need to specify

		"clean": {
			description: "Clean build artifacts"
			command: "echo 'Cleaning...'"
			cache: false
		}
		"format": {
			description: "Format code"
			command: "echo 'Formatting code...'"
			cache: false
		}
		"version": {
			description: "Show version"
			command: "echo 'Version 1.0.0'"
		}
	}

	// Nested groups example
	"release": {
		description: "Full release process"
		mode: "workflow"

		"quality": {
			description: "Quality checks"
			mode: "parallel"

			"lint": {
				command: "echo 'Linting...'"
			}
			"test": {
				command: "echo 'Testing...'"
			}
			"audit": {
				command: "echo 'Security audit...'"
			}
		}

		"build": {
			description: "Build for all platforms"
			mode: "parallel"
			dependencies: ["quality"]

			"linux": {
				command: "echo 'Building for Linux...'"
				outputs: ["target/linux/app"]
			}
			"macos": {
				command: "echo 'Building for macOS...'"
				outputs: ["target/macos/app"]
			}
			"windows": {
				command: "echo 'Building for Windows...'"
				outputs: ["target/windows/app.exe"]
			}
		}

		"publish": {
			description: "Publish release"
			mode: "sequential"
			dependencies: ["build"]

			"tag": {
				command: "echo 'Creating git tag...'"
			}
			"upload": {
				command: "echo 'Uploading artifacts...'"
			}
			"announce": {
				command: "echo 'Release published!'"
			}
		}
	}

	// Example of using task groups as dependencies
	"package": {
		description: "Package the application after running CI checks"
		command: "echo 'Creating package...'"
		dependencies: ["ci"] // Depends on the entire CI group
		outputs: ["package.tar.gz"]
	}

	"deploy": {
		description: "Deploy the packaged application"
		command: "echo 'Deploying to production...'"
		dependencies: ["package", "assets"] // Depends on packaging and all assets being ready
	}

	// Another example with nested groups
	"full-release": {
		description: "Complete release process"
		command: "echo 'Starting full release...'"
		dependencies: ["release"] // Depends on the entire release group (which includes nested groups)
	}
}
