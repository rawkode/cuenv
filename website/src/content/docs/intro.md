---
title: What is cuenv?
description: Type-safe environment management powered by CUE
---

Let's be honest. You're here because you want more from your development environment. You want type safety, validation, and tools that understand your configuration is more than just strings.

## Why CUE for Environment Configuration?

**Shell scripts** work great for automation, but configuration deserves a purpose-built language.

**CUE** was designed specifically for configuration:

- Type system that catches errors before runtime
- Schema validation built into the language
- Unification that makes inheritance natural
- Extensible beyond just environment variables

## What Makes cuenv Different

cuenv leverages CUE's power to go beyond simple environment management:

- Define environment variables with real types and constraints
- Create tasks and workflows alongside your configuration
- Integrate secrets without custom scripts
- Validate everything at load time

## Core Features

- **Type-Safe Configuration**: `PORT: 3000` is an int. CUE validates it before it loads.
- **Native Task Integration**: Define and execute tasks alongside your configuration.
- **Efficient**: State is stored in environment variables, not files. No I/O on every prompt.
- **Beyond Environment**: Define tasks, workflows, and more in the same file.

## Real Examples

### Type Safety That Actually Helps

```cue
// This FAILS at load time, not in production
PORT: "three thousand"  // Error: conflicting values "three thousand" and int

// This works
PORT: 3000
WORKERS: 4
TIMEOUT: 30.5

// With constraints
PORT: int & >=1024 & <=65535  // Must be valid port number
ENVIRONMENT: "dev" | "staging" | "prod"  // Must be one of these
```

### Hierarchical Config (That Makes Sense)

```bash
~/work/
  env.cue         # ORG: "acme", GITHUB_TOKEN: "op://..."

  backend/
    env.cue       # TEAM: "backend", AWS_REGION: "us-east-1"

    api/
      env.cue     # SERVICE: "api", PORT: 8080
      # Has all: ORG, GITHUB_TOKEN, TEAM, AWS_REGION, SERVICE, PORT
```

### Secrets Without The Drama

```cue
// In env.cue
DATABASE_URL: "postgres://user:password@localhost/dev"  // Local dev

// Run your app
cuenv exec -- npm start  # Execute with loaded environment

// Check what's loaded
cuenv env status
```

## How It Actually Works

1. You `cd` into a directory with `env.cue`
2. cuenv loads it, validates it, and sets your environment variables.
3. You `cd` out, and cuenv restores the previous environment.

Behind the scenes:

- Shell hooks catch directory changes.
- CUE validates and evaluates your config.
- State is stored in `CUENV_*` environment variables (no file I/O).

## cuenv + direnv: Better Together

Both tools have their strengths:

| Feature               | cuenv                 | direnv             |
| --------------------- | --------------------- | ------------------ |
| **Config Language**   | CUE (purpose-built)   | Shell (flexible)   |
| **Type Validation**   | Built-in              | Script it yourself |
| **Secret Management** | Native integration    | Custom scripts     |
| **Approval Method**   | `cuenv env allow` command | File path          |
| **Performance**       | Env var state         | File-based         |
| **File Watching**     | Automatic             | `watch_file`       |
| **Extensibility**     | Tasks, workflows      | Shell commands     |
| **Platform Support**  | Cross-platform        | Unix-focused       |

## Who Should Use cuenv?

**Perfect for:**

- Teams wanting type-safe configuration
- Projects with complex environment hierarchies
- Developers who value validation over flexibility
- Anyone managing secrets across environments
- Projects that need more than just env vars (tasks, workflows)

**Consider direnv if:**

- You need maximum flexibility with shell scripts
- Your configuration logic is complex and procedural
- You're already invested in `.envrc` workflows

## Get Started

```bash
# Install it
cargo install cuenv

# Hook it up
eval "$(cuenv shell init bash)"

# Try it
echo 'package env
PORT: 8080
DATABASE_URL: "postgres://localhost/dev"' > env.cue

# That's it
echo $PORT  # 8080
```

Next: [See real examples](/quickstart/) or [dive into the details](/guides/cue-format/)
