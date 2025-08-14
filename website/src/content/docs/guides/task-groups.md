---
title: Task Groups and Execution Modes
description: Organize and execute multiple tasks with different strategies
---

# Task Groups and Execution Modes

cuenv supports organizing tasks into groups with different execution strategies. This allows you to structure complex workflows, run parallel tasks, or create simple task collections.

## Task Group Types

### Group Mode (Default)

Simple task collection for organization without automatic execution.

```cue
package env

tasks: {
    development: {
        description: "Development tasks"
        mode: "group"
        
        start: {
            description: "Start development server"
            command: "npm run dev"
        }
        
        test: {
            description: "Run tests in watch mode"
            command: "npm run test:watch"
        }
        
        lint: {
            description: "Run linter"
            command: "npm run lint"
        }
    }
}
```

**Usage:**

```bash
# List tasks in the group
cuenv task development

# Execute individual tasks
cuenv task development start
cuenv task development.start  # Alternative syntax
```

### Sequential Mode

Execute tasks one after another in definition order.

```cue
package env

tasks: {
    ci: {
        description: "CI pipeline"
        mode: "sequential"
        
        install: {
            description: "Install dependencies"
            command: "npm install"
        }
        
        lint: {
            description: "Run linter"
            command: "npm run lint"
        }
        
        test: {
            description: "Run tests"
            command: "npm test"
        }
        
        build: {
            description: "Build application"
            command: "npm run build"
        }
    }
}
```

**Usage:**

```bash
# Execute all tasks in sequence
cuenv task ci

# Tasks run in order: install → lint → test → build
# If any task fails, execution stops
```

### Parallel Mode

Execute all tasks simultaneously for maximum speed.

```cue
package env

tasks: {
    checks: {
        description: "Quality checks"
        mode: "parallel"
        
        lint: {
            description: "Run linter"
            command: "npm run lint"
        }
        
        typecheck: {
            description: "Type checking"
            command: "npm run typecheck"
        }
        
        test: {
            description: "Run tests"
            command: "npm test"
        }
        
        audit: {
            description: "Security audit"
            command: "npm audit"
        }
    }
}
```

**Usage:**

```bash
# Execute all tasks in parallel
cuenv task checks

# All tasks start simultaneously
# Execution completes when all tasks finish
```

### Workflow Mode

Execute tasks based on dependency graph (DAG) for complex workflows.

```cue
package env

tasks: {
    deploy: {
        description: "Deployment workflow"
        mode: "workflow"
        
        test: {
            description: "Run tests"
            command: "npm test"
        }
        
        build: {
            description: "Build application"
            command: "npm run build"
            dependencies: ["test"]
        }
        
        package: {
            description: "Package for deployment"
            command: "docker build -t myapp ."
            dependencies: ["build"]
        }
        
        deploy: {
            description: "Deploy to production"
            command: "kubectl apply -f k8s/"
            dependencies: ["package"]
        }
        
        verify: {
            description: "Verify deployment"
            command: "curl -f https://myapp.com/health"
            dependencies: ["deploy"]
        }
    }
}
```

**Usage:**

```bash
# Execute workflow
cuenv task deploy

# Execution order determined by dependencies:
# test → build → package → deploy → verify
```

## Task Group Configuration

### Group Description

Add descriptions to document the purpose of task groups:

```cue
tasks: {
    frontend: {
        description: "Frontend development tasks for React application"
        mode: "group"
        
        // ... tasks
    }
}
```

### Nested Groups

Create hierarchical task organization:

```cue
tasks: {
    app: {
        description: "Application tasks"
        mode: "group"
        
        frontend: {
            description: "Frontend tasks"
            mode: "group"
            
            build: {
                command: "npm run build"
            }
            
            test: {
                command: "npm run test"
            }
        }
        
        backend: {
            description: "Backend tasks"
            mode: "sequential"
            
            compile: {
                command: "cargo build"
            }
            
            test: {
                command: "cargo test"
            }
        }
    }
}
```

**Usage:**

```bash
# List all app tasks
cuenv task app

# Execute frontend group
cuenv task app frontend

# Execute specific task
cuenv task app frontend build
cuenv task app.frontend.build  # Alternative syntax
```

## Execution Strategies

### Error Handling

#### Sequential Mode

- Stops immediately on first failure
- Later tasks are skipped
- Exit code reflects the failed task

#### Parallel Mode

- All tasks run to completion
- Final exit code is non-zero if any task failed
- Shows summary of successes and failures

#### Workflow Mode

- Stops execution branch on failure
- Other independent branches continue
- Dependent tasks are skipped

### Output Management

Different output formats handle task groups differently:

#### Spinner Format (Default)

```bash
cuenv task ci --output spinner
# ⏳ Running CI pipeline...
# ✓ install completed
# ⏳ lint running...
# ✓ lint completed
# ⏳ test running...
```

#### TUI Format

```bash
cuenv task ci --output tui
# Shows interactive interface with:
# - Task progress bars
# - Real-time logs
# - Dependency visualization
```

#### Tree Format

```bash
cuenv task ci --output tree
# ci
# ├── install ✓
# ├── lint ✓  
# ├── test ⏳
# └── build ⏸️
```

## Best Practices

### 1. Choose Appropriate Modes

- **Group**: For related tasks that are usually run individually
- **Sequential**: For CI/CD pipelines where order matters
- **Parallel**: For independent quality checks
- **Workflow**: For complex builds with dependencies

### 2. Design for Failure

```cue
tasks: {
    robust_ci: {
        mode: "sequential"
        
        setup: {
            description: "Setup that must succeed"
            command: "setup.sh"
        }
        
        checks: {
            description: "Parallel quality checks"
            mode: "parallel"
            
            lint: { command: "npm run lint" }
            test: { command: "npm test" }
            security: { command: "npm audit" }
        }
        
        build: {
            description: "Build only if checks pass"
            command: "npm run build"
            dependencies: ["checks"]
        }
    }
}
```

### 3. Use Clear Names

```cue
// Good: Descriptive names
tasks: {
    quality_checks: { mode: "parallel" }
    build_pipeline: { mode: "sequential" }
    deployment_workflow: { mode: "workflow" }
}

// Avoid: Generic names
tasks: {
    tasks1: { mode: "parallel" }
    group: { mode: "sequential" }
    stuff: { mode: "workflow" }
}
```

### 4. Document Dependencies

```cue
tasks: {
    publish: {
        mode: "workflow"
        
        test: {
            description: "Ensure code quality before publish"
            command: "npm test"
        }
        
        build: {
            description: "Create production build"
            command: "npm run build"
            dependencies: ["test"]  # Explicit dependency
        }
        
        publish: {
            description: "Publish to npm registry"
            command: "npm publish"
            dependencies: ["build"]  # Must build first
        }
    }
}
```

## Command Reference

### List Group Tasks

```bash
# List all tasks
cuenv task

# List tasks in specific group
cuenv task <group_name>

# Verbose output with descriptions
cuenv task <group_name> --verbose
```

### Execute Groups

```bash
# Execute group (behavior depends on mode)
cuenv task <group_name>

# Execute specific task in group
cuenv task <group_name> <task_name>
cuenv task <group_name>.<task_name>

# With options
cuenv task <group_name> --output tui --audit
```

### Environment and Capabilities

```bash
# Execute with environment
cuenv task ci -e production

# Execute with capabilities
cuenv task deploy -c network -c secrets

# Combined
cuenv task deploy -e production -c network -c secrets
```

## Examples

### Frontend Development

```cue
tasks: {
    dev: {
        description: "Frontend development workflow"
        mode: "workflow"
        
        deps: {
            description: "Install dependencies"
            command: "npm install"
        }
        
        generate: {
            description: "Generate types"
            command: "npm run codegen"
            dependencies: ["deps"]
        }
        
        dev: {
            description: "Start development server"
            command: "npm run dev"
            dependencies: ["generate"]
        }
    }
    
    quality: {
        description: "Quality assurance checks"
        mode: "parallel"
        
        lint: {
            description: "ESLint check"
            command: "npm run lint"
        }
        
        typecheck: {
            description: "TypeScript check"
            command: "npm run typecheck"
        }
        
        test: {
            description: "Unit tests"
            command: "npm test"
        }
        
        e2e: {
            description: "End-to-end tests"
            command: "npm run test:e2e"
        }
    }
}
```

### Backend Services

```cue
tasks: {
    microservices: {
        description: "Microservice management"
        mode: "group"
        
        auth: {
            description: "Authentication service tasks"
            mode: "sequential"
            
            build: { command: "docker build -t auth-service ." }
            test: { command: "docker run --rm auth-service test" }
            deploy: { command: "kubectl apply -f auth-service.yaml" }
        }
        
        api: {
            description: "API gateway tasks"
            mode: "sequential"
            
            build: { command: "docker build -t api-gateway ." }
            test: { command: "docker run --rm api-gateway test" }
            deploy: { command: "kubectl apply -f api-gateway.yaml" }
        }
    }
}
```

Usage:

```bash
# Deploy all services
cuenv task microservices auth
cuenv task microservices api

# Or individually
cuenv task microservices.auth.deploy
cuenv task microservices.api.deploy
```