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
	// Parallel with dependencies - object structure with named tasks
	"ci": {
		description: "CI workflow with dependency graph"
		// Object structure automatically enables parallel execution with dependencies
		tasks: {
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
	}

	// Sequential mode - array structure with ordered tasks
	"deploy": {
		description: "Deployment process"
		// Array structure automatically enables sequential execution in order
		tasks: [
			{
				description: "Backup current deployment"
				command: "echo 'Creating backup...'"
			},
			{
				description: "Upload new version"
				command: "echo 'Uploading files...'"
			},
			{
				description: "Run database migrations"
				command: "echo 'Running migrations...'"
			},
			{
				description: "Verify deployment"
				command: "echo 'Verifying deployment...'"
			},
			{
				description: "Send notifications"
				command: "echo 'Deployment complete!'"
			}
		]
	}

	// Parallel mode - object structure with named tasks
	"assets": {
		description: "Build all assets in parallel"
		// Object structure enables parallel execution of all tasks
		tasks: {
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
	}

	// Parallel mode - object structure with named utility tasks
	"utils": {
		description: "Utility tasks"
		// Object structure enables parallel execution of utility tasks
		tasks: {
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
	}

	// Nested task groups example with mixed execution modes
	"release": {
		description: "Full release process"
		// Object structure with dependencies creates a dependency graph workflow
		tasks: {
			"quality": {
				description: "Quality checks"
				// Object structure enables parallel execution of quality checks
				tasks: {
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
			}

			"build": {
				description: "Build for all platforms"
				dependencies: ["quality"]
				// Object structure enables parallel builds for different platforms
				tasks: {
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
			}

			"publish": {
				description: "Publish release"
				dependencies: ["build"]
				// Array structure ensures sequential execution of publish steps
				tasks: [
					{
						description: "Create git tag"
						command: "echo 'Creating git tag...'"
					},
					{
						description: "Upload artifacts"
						command: "echo 'Uploading artifacts...'"
					},
					{
						description: "Announce release"
						command: "echo 'Release published!'"
					}
				]
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

	"production-deploy": {
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
