# Monorepo Support

cuenv provides comprehensive support for monorepo structures, allowing you to manage environment configurations and tasks across multiple packages in a single repository.

## Overview

In a monorepo setup, cuenv can:
- Discover all CUE packages in your repository
- Execute tasks with cross-package dependencies
- Stage outputs from one package as inputs to another
- Provide isolated execution environments
- Maintain package-specific environment scopes

## Configuration

### Module Root

A monorepo is identified by the presence of a `cue.mod` directory containing a `module.cue` file:

```cue
// cue.mod/module.cue
module: "example.com/mymonorepo"
```

### Package Structure

Each package in the monorepo contains its own `env.cue` file:

```
myrepo/
├── cue.mod/
│   └── module.cue
├── env.cue                 # Root package
├── services/
│   ├── api/
│   │   └── env.cue        # services:api package
│   └── web/
│       └── env.cue        # services:web package
└── tools/
    └── deploy/
        └── env.cue        # tools:deploy package
```

## Package Naming

Packages are named hierarchically based on their path from the module root:
- Root package: `root`
- Nested packages: `parent:child` (e.g., `services:api`)
- Deeply nested: `grandparent:parent:child` (e.g., `tools:ci:deploy`)

## Cross-Package Task Dependencies

Tasks can depend on tasks from other packages using the package:task notation:

```cue
// tools/deploy/env.cue
package env

tasks: {
    "deploy": {
        description: "Deploy all services"
        command: "deploy.sh"
        dependencies: [
            "services:api:build",
            "services:web:build"
        ]
    }
}
```

## Task Outputs and Inputs

### Declaring Outputs

Tasks can declare outputs that other tasks can consume:

```cue
// services/api/env.cue
tasks: {
    "build": {
        command: "go build -o bin/api"
        outputs: ["bin/api"]
    }
}
```

### Using Outputs as Inputs

Tasks can reference outputs from other packages:

```cue
// tools/deploy/env.cue
tasks: {
    "deploy": {
        command: "deploy.sh"
        dependencies: ["services:api:build"]
        inputs: ["services:api:build:bin/api"]
    }
}
```

## Staged Dependencies

When a task declares inputs, cuenv stages these dependencies in an isolated environment:

1. **Automatic Staging**: Dependencies are copied or symlinked to a temporary staging directory
2. **Environment Variables**: Staged paths are exposed via environment variables
3. **Isolation**: Each task execution gets its own staging environment

### Environment Variable Format

Staged inputs are available through environment variables:
- Pattern: `CUENV_INPUT_{PACKAGE}_{TASK}_{OUTPUT}`
- Example: `CUENV_INPUT_SERVICES_API_BUILD_BIN_API`

Special characters are converted to underscores:
- `:` → `_`
- `-` → `_`
- `/` → `_`
- `.` → `_`

## Task Execution

### Running Cross-Package Tasks

Execute any task from anywhere in the monorepo:

```bash
# Execute from repository root
cuenv run services:api:build

# Execute from any subdirectory
cd tools/deploy
cuenv run services:api:build

# Execute local task
cuenv run deploy
```

### Execution Order

Tasks are executed in topological order based on their dependencies:
1. Leaf tasks (no dependencies) execute first
2. Dependent tasks execute after their dependencies complete
3. Circular dependencies are detected and prevented

### Task Caching

Tasks are cached within a single execution session:
- Each task executes at most once
- Diamond dependencies are handled efficiently
- Cache is cleared between independent executions

## Package Discovery

### Listing Packages

Discover all packages in the repository:

```bash
cuenv discover
```

Output:
```
Found 4 packages:
  root
  services:api
  services:web
  tools:deploy
```

### Detailed Package Information

View detailed information about packages:

```bash
cuenv discover --dump
```

This shows:
- Package paths
- Environment variables
- Task definitions
- Dependencies

## Listing Tasks

List all available tasks across the monorepo:

```bash
cuenv run
```

Output:
```
Available tasks:

  Package: root
    root:clean: Clean all build artifacts

  Package: services:api
    services:api:build: Build API service
    services:api:test: Run API tests

  Package: services:web
    services:web:build: Build web application
    services:web:test: Run web tests

  Package: tools:deploy
    tools:deploy:deploy: Deploy all services
```

## Environment Inheritance

Packages can inherit environment variables from parent packages:

1. **Root Variables**: Available to all packages
2. **Package Variables**: Override parent values
3. **Isolation**: Each package maintains its own scope

Example:
```cue
// env.cue (root)
env: {
    LOG_LEVEL: "info"
    API_URL: "https://api.example.com"
}

// services/api/env.cue
env: {
    LOG_LEVEL: "debug"  // Overrides root value
    PORT: "8080"        // Package-specific
}
```

## Security Considerations

### Task Isolation

- Tasks execute in their working directory
- Staged inputs provide read-only access to dependencies
- Environment variables are isolated per task

### Access Restrictions

Tasks can specify security restrictions:

```cue
tasks: {
    "build": {
        command: "build.sh"
        restrictions: {
            network: ["deny"]
            filesystem: {
                read: [".", "/tmp"]
                write: ["./dist"]
            }
        }
    }
}
```

## Best Practices

### 1. Clear Output Declaration

Always declare task outputs explicitly:
```cue
outputs: ["dist/app.js", "dist/app.css"]
```

### 2. Semantic Task Names

Use descriptive task names:
- `build` - Build artifacts
- `test` - Run tests
- `deploy` - Deploy service
- `clean` - Clean artifacts

### 3. Dependency Management

- Keep dependency graphs shallow
- Avoid circular dependencies
- Use inputs only when needed

### 4. Package Organization

- Group related services together
- Keep tools separate from services
- Use clear, hierarchical naming

### 5. Documentation

Document tasks with descriptions:
```cue
tasks: {
    "deploy": {
        description: "Deploy service to production"
        command: "deploy.sh"
    }
}
```

## Troubleshooting

### Task Not Found

If a cross-package task isn't found:
1. Verify the package path is correct
2. Check the task exists in the target package
3. Ensure the env.cue file is valid CUE

### Circular Dependencies

cuenv detects circular dependencies:
```
Error: Circular dependency detected: services:api:build
```

Resolution:
1. Review task dependencies
2. Restructure to remove cycles
3. Consider extracting common dependencies

### Missing Outputs

If a task output isn't found:
1. Ensure the task declares the output
2. Verify the output path is correct
3. Check the task actually creates the output

### Staging Issues

If staged inputs aren't available:
1. Check environment variable names
2. Verify the input reference format
3. Ensure the dependency task has completed

## Examples

See the [`examples/monorepo`](../examples/monorepo) directory for a complete working example demonstrating:
- Package discovery
- Cross-package dependencies
- Staged inputs
- Task execution
- Environment inheritance