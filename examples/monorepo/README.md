# Monorepo Example

This example demonstrates cuenv's support for monorepo structures with cross-package task dependencies.

## Structure

```
monorepo/
├── cue.mod/           # CUE module root
│   └── module.cue
├── env.cue            # Root package configuration
├── projects/
│   ├── frontend/      # Frontend package
│   │   └── env.cue
│   └── backend/       # Backend package
│       └── env.cue
└── tools/
    ├── ci/            # CI/CD tools package
    │   └── env.cue
    └── scripts/       # Scripts package
        └── env.cue
```

## Features Demonstrated

### 1. Package Discovery

Discover all packages in the monorepo:

```bash
cuenv discover
```

This will list all packages with their hierarchical names:

- `root` - Root package
- `projects:frontend` - Frontend project
- `projects:backend` - Backend project
- `tools:ci` - CI/CD tools
- `tools:scripts` - Utility scripts

### 2. Cross-Package Dependencies

The `tools:ci:deploy` task depends on both frontend and backend builds:

```cue
tasks: {
    "deploy": {
        command: "deployer"
        dependencies: ["projects:frontend:build", "projects:backend:build"]
        inputs: ["projects:frontend:build#dist", "projects:backend:build#bin/server"]
    }
}
```

### 3. Task Execution

Execute a task with cross-package dependencies:

```bash
# From any directory in the monorepo:
cuenv run tools:ci:deploy

# Or use the short form from within a package:
cd tools/ci
cuenv run deploy
```

### 4. Staged Inputs

Tasks can reference outputs from other packages:

```cue
inputs: ["projects:frontend:build#dist"]
```

These inputs are staged in an isolated environment and made available via environment variables:

- `CUENV_INPUT_PROJECTS_FRONTEND_BUILD_DIST`

### 5. Task Outputs

Tasks can declare outputs that other tasks can depend on:

```cue
tasks: {
    "build": {
        command: "vite build"
        outputs: ["dist"]
    }
}
```

## Usage Examples

### List all tasks across the monorepo:

```bash
cuenv run
```

### Execute the frontend build:

```bash
cuenv run projects:frontend:build
```

### Execute the deploy task (builds all dependencies):

```bash
cuenv run tools:ci:deploy
```

### View package details:

```bash
cuenv discover --dump
```

## Environment Inheritance

Packages inherit environment variables from their parent packages:

- Root variables are available to all packages
- Package-specific variables override parent values
- Each package maintains its own environment scope

## Security

Tasks can specify security restrictions that apply during execution:

- File system access restrictions
- Network access control
- Sandboxing via Landlock (Linux)

## Benefits

1. **Type-safe configuration**: CUE provides schema validation
2. **Dependency management**: Automatic task ordering
3. **Isolation**: Staged inputs prevent side effects
4. **Discoverability**: Easy to find all tasks and packages
5. **Reproducibility**: Consistent execution across environments
