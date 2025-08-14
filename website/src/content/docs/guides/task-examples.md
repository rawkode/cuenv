---
title: Task System Examples
description: Learn how to use cuenv's powerful task system with Bazel-style build caching
---

# Tasks Example with Build Cache

This example demonstrates cuenv's powerful task system with the new Bazel-style build cache support.

## Features Demonstrated

- **Task Definition**: Define tasks with descriptions, commands/scripts, and dependencies
- **Build Cache**: Bazel-style caching with content-based cache keys and automatic invalidation
- **Dependency Resolution**: Tasks automatically execute their dependencies in the correct order
- **Parallel Execution**: Independent tasks run concurrently for better performance
- **Environment Inheritance**: Tasks inherit all environment variables from env.cue
- **Input/Output Tracking**: Track file dependencies for intelligent cache invalidation

## Build Cache Features

The build cache system provides enterprise-grade caching with:

- **Content-based Cache Keys**: SHA256 hashing of task configuration + input file contents
- **Input Tracking**: Automatic cache invalidation when input files change
- **Output Validation**: Cached tasks verified by checking output file existence and hashes
- **Dependency-aware**: Works seamlessly with task dependency resolution
- **Custom Cache Keys**: Support for user-defined cache keys with `cacheKey` field
- **Selective Caching**: Tasks can disable caching with `cache: false`

## Usage

### List Available Tasks

```bash
cuenv run                    # List all tasks and groups
cuenv task                   # Alternative command
```

### List Tasks in a Group

```bash
cuenv task fmt              # List tasks in the 'fmt' group
```

### Execute a Task (with caching)

```bash
cuenv run lint              # First run executes
cuenv run lint              # Second run uses cache
```

### Execute a Task from a Group

```bash
cuenv task fmt check        # Run the 'check' task from 'fmt' group
cuenv task fmt apply        # Run the 'apply' task from 'fmt' group
```

### Execute Task with Dependencies

```bash
cuenv run build            # Executes lint → test → build (if not cached)
```

### Clear Build Cache

```bash
cuenv clear-cache
```

## Task Execution Modes

Task groups can now have different execution modes that control how subtasks are run:

- **workflow** (or **dag**): Tasks execute based on their dependency relationships
- **sequential**: Tasks execute one after another in definition order
- **parallel**: All tasks execute simultaneously
- **group** (default): Simple collection of tasks with no special behavior

### Example: Workflow Mode

```cue
tasks: {
    "ci": {
        description: "CI workflow"
        mode: "workflow"  // Tasks run based on dependencies

        "lint": {
            command: "cargo clippy"
        }
        "test": {
            command: "cargo test"
            dependencies: ["lint"]  // Runs after lint
        }
        "build": {
            command: "cargo build --release"
            dependencies: ["test"]  // Runs after test
        }
    }
}
```

### Example: Sequential Mode

```cue
tasks: {
    "deploy": {
        description: "Deployment process"
        mode: "sequential"  // Tasks run one after another

        "backup": { command: "pg_dump mydb > backup.sql" }
        "upload": { command: "rsync -av dist/ server:/app/" }
        "migrate": { command: "ssh server 'cd /app && migrate up'" }
        "verify": { command: "curl -f https://myapp.com/health" }
    }
}
```

### Example: Parallel Mode

```cue
tasks: {
    "assets": {
        description: "Build assets in parallel"
        mode: "parallel"  // All tasks run simultaneously

        "css": { command: "sass compile styles.scss" }
        "js": { command: "esbuild bundle app.js" }
        "images": { command: "imagemin optimize images/*" }
    }
}
```

## Task Definition Schema

### Single Task

```cue
tasks: {
    "task-name": {
        description: "Human readable description"
        command: "shell command to execute"           // OR
        script: "multi-line shell script"             // (mutually exclusive)
        dependencies: ["other-task1", "other-task2"] // Optional
        workingDir: "./relative/path"                 // Optional
        shell: "bash"                                 // Optional (default: "sh")
        cache: true                                   // Enable build cache
        inputs: ["src/**/*.go", "tests/**/*"]         // Files to track for cache invalidation
        outputs: ["bin/myapp", "build/artifacts/*"]   // Output files for cache validation
        cacheKey: "custom-key"                        // Optional custom cache key
    }
}
```

### Task Groups

Tasks can be organized into groups with different execution modes:

```cue
tasks: {
    fmt: {
        description: "Code formatting tasks"
        mode: "sequential"  // Optional: specify execution mode
        check: {
            description: "Check formatting without changes"
            command: "treefmt"
            args: ["--fail-on-change"]
        }
        apply: {
            description: "Apply formatting changes"
            command: "treefmt"
        }
    }
}
```

This creates:

- `fmt` - A task group that runs tasks sequentially
- `fmt.check` - Check formatting (accessed as `cuenv task fmt check`)
- `fmt.apply` - Apply formatting (accessed as `cuenv task fmt apply`)

### Nested Task Groups

Task groups can be nested to create complex workflows:

```cue
tasks: {
    "release": {
        mode: "workflow"

        "quality": {
            mode: "parallel"  // Run all quality checks in parallel
            "lint": { command: "cargo clippy" }
            "test": { command: "cargo test" }
            "audit": { command: "cargo audit" }
        }

        "build": {
            dependencies: ["quality"]  // Wait for all quality checks
            command: "cargo build --release"
        }
    }
}
```

### Dependency Resolution with Groups

When referencing dependencies:

- Use task name for tasks in the same group: `dependencies: ["lint"]`
- Use group name to depend on entire group completion: `dependencies: ["quality"]`
- Use qualified name for specific task in a group: `dependencies: ["quality:lint"]`

## Example Tasks

- **lint**: Lints the code (cached, tracks `src/*`)
- **test**: Runs tests (cached, depends on lint, tracks `src/*` and `tests/*`)
- **build**: Builds the project (cached, depends on test, outputs to `build/app`)
- **deploy**: Deploys the application (not cached, depends on build)
- **clean**: Cleans build artifacts (not cached, always runs)
- **script-example**: Demonstrates multi-line script with caching

## Cache Behavior Examples

### Initial Run

```bash
$ cuenv run build
→ Executing task 'lint'
Linting code...
→ Executing task 'test'
Running tests...
→ Executing task 'build'
Building project...
```

### Subsequent Run (All Cached)

```bash
$ cuenv run build
✓ Task 'lint' found in cache, skipping execution
✓ Task 'test' found in cache, skipping execution
✓ Task 'build' found in cache, skipping execution
```

### After Modifying Input File

```bash
$ echo "// comment" >> src/main.rs
$ cuenv run build
→ Executing task 'lint'       # Cache invalidated due to input change
Linting code...
✓ Task 'test' found in cache, skipping execution  # Different inputs
→ Executing task 'build'      # Cache invalidated due to dependency change
Building project...
```

## Cache Storage

- **Location**: `~/.cache/cuenv/tasks/`
- **Format**: JSON files with SHA256 filenames
- **Contents**: Exit code, output file hashes, execution timestamp
- **Cleanup**: Use `cuenv clear-cache` to remove all cached results

## Error Handling

- **Missing Task**: Error if requested task doesn't exist
- **Missing Dependency**: Error if task depends on non-existent task
- **Circular Dependencies**: Detected and reported as error
- **Task Failure**: Failed tasks are not cached, stops execution chain
- **Cache Corruption**: Automatic cache invalidation on file corruption

## Environment Variables

All tasks automatically inherit environment variables defined in the env.cue file:

- `DATABASE_URL`: postgres://localhost/myapp
- `API_KEY`: test-api-key
- `PORT`: 3000

These are available as `$DATABASE_URL`, `$API_KEY`, `$PORT` in task commands/scripts.

## Performance Benefits

The build cache dramatically improves development workflows:

- **Skip redundant compilation**: Only rebuild when source files change
- **Faster CI/CD**: Reuse artifacts across pipeline stages
- **Incremental builds**: Smart dependency tracking
- **Team collaboration**: Shared cache improves team productivity

This makes cuenv ideal for monorepos and complex build pipelines where build performance is critical.

## Performance Analysis and Debugging

### Chrome Trace Output

cuenv can generate Chrome trace files for detailed performance analysis of task execution. This is particularly useful for optimizing complex task graphs and identifying bottlenecks.

```bash
# Generate trace file during task execution
cuenv task build --trace-output

# The trace file is saved as cuenv-trace-<timestamp>.json
ls cuenv-trace-*.json
```

#### Viewing Trace Files

Open the generated trace file in Chrome's tracing tool:

1. Open Chrome and navigate to `chrome://tracing/`
2. Click "Load" and select your trace file
3. Use the interface to explore task execution timelines

#### What the Trace Shows

The Chrome trace includes:

- **Task Execution Timeline**: Start/end times for each task
- **Dependency Resolution**: How dependencies are resolved
- **Cache Lookups**: Time spent checking and retrieving cached results
- **Parallel Execution**: Visual representation of concurrent task execution
- **Total Execution Time**: Overall performance metrics

#### Example Trace Analysis

```bash
# Execute a complex task graph with tracing
cuenv task deploy --trace-output

# Look for performance insights:
# - Which tasks take the longest?
# - Are tasks running in parallel as expected?
# - How much time is spent on cache operations?
# - Where are the bottlenecks in dependency chains?
```

This is especially valuable for:
- **CI/CD Optimization**: Identify slow steps in build pipelines
- **Monorepo Performance**: Analyze task execution across multiple packages
- **Cache Effectiveness**: Measure cache hit rates and lookup times
- **Parallel Execution Tuning**: Optimize task parallelization

### Debug Output

For additional debugging information:

```bash
# Enable verbose logging
RUST_LOG=debug cuenv task build

# Enable cuenv-specific debug output
CUENV_DEBUG=1 cuenv task build
```

## Related Guides

- [Cache System](/guides/cache-system/) - Deep dive into caching architecture
- [Remote Cache Configuration](/guides/remote-cache-configuration/) - Distributed caching
- [Commands Reference](/reference/commands/) - Complete CLI reference
