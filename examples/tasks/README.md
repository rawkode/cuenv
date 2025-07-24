# Task Support Example

This example demonstrates cuenv's task support functionality, which allows you to define and execute project-specific tasks with dependency management.

## Features Demonstrated

- **Task Definition**: Define tasks with descriptions, commands/scripts, and dependencies
- **Dependency Resolution**: Tasks automatically execute their dependencies in the correct order
- **Parallel Execution**: Independent tasks run concurrently for better performance
- **Environment Inheritance**: Tasks inherit all environment variables from env.cue
- **Command vs Script**: Support for both single commands and multi-line scripts

## Usage

### List Available Tasks
```bash
cuenv run
```

### Execute a Task
```bash
cuenv run <task_name>
```

### Execute Task with Arguments
```bash
cuenv run <task_name> -- --arg1 value1 --arg2 value2
```

## Task Definition Schema

```cue
tasks: {
    "task-name": {
        description: "Human readable description"
        command: "shell command to execute"           // OR
        script: "multi-line shell script"             // (mutually exclusive)
        dependencies: ["other-task1", "other-task2"] // Optional
        workingDir: "./relative/path"                 // Optional
        shell: "bash"                                 // Optional (default: "sh")
        inputs: ["src/**/*.go"]                       // Future feature
        outputs: ["bin/myapp"]                        // Future feature
    }
}
```

## Example Tasks in this Directory

- **lint**: Lints the code (no dependencies)
- **test**: Runs tests (depends on lint)
- **build**: Builds the project (depends on test)
- **deploy**: Deploys the application (depends on build) 
- **clean**: Cleans build artifacts (independent)
- **script-example**: Demonstrates multi-line script execution

## Task Execution Flow

When you run `cuenv run deploy`, the execution order will be:

1. **lint** (executes first, no dependencies)
2. **test** (executes after lint completes)
3. **build** (executes after test completes)  
4. **deploy** (executes after build completes)

If multiple tasks have no outstanding dependencies, they execute in parallel.

## Error Handling

- **Missing Task**: Error if requested task doesn't exist
- **Missing Dependency**: Error if task depends on non-existent task
- **Circular Dependencies**: Detected and reported as error
- **Task Failure**: Stops execution and reports which task failed
- **Exit Codes**: Task exit codes are propagated to cuenv

## Environment Variables

All tasks automatically inherit environment variables defined in the env.cue file:
- `DATABASE_URL`: postgres://localhost/myapp
- `API_KEY`: test-api-key  
- `PORT`: 3000

These are available as `$DATABASE_URL`, `$API_KEY`, `$PORT` in task commands/scripts.