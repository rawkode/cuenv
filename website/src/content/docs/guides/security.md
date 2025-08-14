---
title: Security & Sandboxing
description: Learn how to use cuenv's security features to sandbox tasks
---

Cuenv provides powerful security features that allow you to sandbox tasks, restricting their access to the filesystem and network. This is especially useful for running untrusted code or enforcing the principle of least privilege.

## Overview

Security features in cuenv are powered by Linux's Landlock LSM (Linux Security Module), providing kernel-level enforcement of access restrictions. This means that once restrictions are applied, they cannot be bypassed by the sandboxed process.

### Requirements

- **Filesystem restrictions**: Linux kernel 5.13+
- **Network restrictions**: Linux kernel 6.7+
- Other platforms: Security restrictions are silently ignored

## Basic Security Configuration

Tasks can specify security restrictions using the `security` field:

```cue
tasks: {
    "secure-task": {
        command: "your-command"
        security: {
            restrictDisk: true
            restrictNetwork: true
            readOnlyPaths: ["/usr", "/lib", "/bin"]
            readWritePaths: ["/tmp", "./output"]
            allowedHosts: ["api.example.com", "cdn.example.com"]
        }
    }
}
```

## Security Options

### Filesystem Restrictions

- **`restrictDisk`**: Enable filesystem sandboxing
- **`readOnlyPaths`**: List of paths the task can read from
- **`readWritePaths`**: List of paths the task can read from and write to
- **`denyPaths`**: List of paths to explicitly deny (overrides allow rules)

```cue
security: {
    restrictDisk: true
    readOnlyPaths: [
        "/usr",      // System binaries
        "/lib",      // System libraries
        "/etc/ssl",  // SSL certificates
        "./config"   // Application config
    ]
    readWritePaths: [
        "/tmp",      // Temporary files
        "./output",  // Output directory
        "./cache"    // Cache directory
    ]
    denyPaths: [
        "/etc/passwd",  // Explicitly deny sensitive files
        "/etc/shadow"
    ]
}
```

### Network Restrictions

- **`restrictNetwork`**: Enable network sandboxing
- **`allowedHosts`**: List of hostnames the task can connect to

```cue
security: {
    restrictNetwork: true
    allowedHosts: [
        "api.github.com",
        "registry.npmjs.org",
        "pypi.org"
    ]
}
```

**Note**: Network restrictions in Landlock are port-based, not hostname-based. Cuenv resolves hostnames to their IP addresses at restriction time.

## Automatic Security Inference

Cuenv can automatically infer filesystem restrictions based on declared task inputs and outputs:

```cue
tasks: {
    "process-data": {
        command: "python process.py"
        inputs: [
            "./data/input.csv",
            "./scripts/process.py"
        ]
        outputs: [
            "./results/output.json",
            "./logs/"
        ]
        security: {
            inferFromInputsOutputs: true
            // Also adds system paths automatically
            readOnlyPaths: ["/usr", "/lib"]
        }
    }
}
```

When `inferFromInputsOutputs` is enabled:

- Paths in `inputs` get read-only access
- Paths in `outputs` get read-write access
- Parent directories are granted appropriate access
- System paths for executables are included

## Audit Mode

Use audit mode to understand what access your tasks actually need:

```bash
# Run task without restrictions but log all access
cuenv task --audit my-task

# The output will show:
# - Files read
# - Files written
# - Network connections made
```

This is invaluable for creating minimal security configurations.

## Best Practices

### 1. Start with Audit Mode

Always use audit mode first to understand actual access patterns:

```bash
cuenv task --audit build
# Analyze the output to see what paths are accessed
```

### 2. Use Minimal Permissions

Grant only the access that's absolutely necessary:

```cue
security: {
    restrictDisk: true
    // Don't do this:
    readWritePaths: ["/"]

    // Do this instead:
    readOnlyPaths: ["/usr/bin", "/lib", "./src"]
    readWritePaths: ["./build", "/tmp"]
}
```

### 3. Combine with Other Security Measures

Landlock complements but doesn't replace other security practices:

- Use proper file permissions
- Validate inputs
- Run with minimal user privileges
- Use container isolation when appropriate

### 4. Test Restrictions Thoroughly

Always test that your security configurations work as expected:

```cue
tasks: {
    "test-restrictions": {
        command: "cat /etc/passwd || echo 'Access denied as expected'"
        security: {
            restrictDisk: true
            readOnlyPaths: ["/usr/bin"]
            // /etc/passwd not allowed - should fail
        }
    }
}
```

## Examples

### Build Task with Minimal Access

```cue
tasks: {
    "build": {
        description: "Build the project securely"
        command: "npm run build"
        security: {
            restrictDisk: true
            restrictNetwork: true
            readOnlyPaths: [
                "/usr", "/lib", "/bin",
                "./src", "./package.json",
                "./node_modules"
            ]
            readWritePaths: [
                "./dist",
                "/tmp"
            ]
            allowedHosts: ["registry.npmjs.org"]
        }
    }
}
```

### Data Processing with Inferred Security

```cue
tasks: {
    "process": {
        description: "Process CSV files"
        command: "python analyze.py"
        inputs: ["./data/*.csv", "./analyze.py"]
        outputs: ["./results/"]
        security: {
            inferFromInputsOutputs: true
            restrictNetwork: true
            // No network access needed
        }
    }
}
```

### Development Server with Network Access

```cue
tasks: {
    "dev": {
        description: "Start development server"
        command: "npm run dev"
        security: {
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "./src"]
            readWritePaths: ["./dist", "/tmp"]
            // No network restrictions for dev server
        }
    }
}
```

## Troubleshooting

### "Permission denied" errors

If tasks fail with permission errors:

1. Run with `--audit` to see what access is needed
2. Check that all required paths are in `readOnlyPaths` or `readWritePaths`
3. Remember that parent directories need at least read access
4. Check for symbolic links that might need additional access

### Network connections failing

1. Ensure the hostname is in `allowedHosts`
2. Check if the service uses multiple hostnames
3. Remember that redirects might go to different hosts
4. Use `--audit` to see actual connection attempts

### Landlock not available

If you see warnings about Landlock not being available:

1. Check kernel version: `uname -r` (need 5.13+ for filesystem, 6.7+ for network)
2. Check if Landlock is enabled: `cat /proc/self/status | grep Seccomp`
3. Security restrictions are silently ignored on unsupported systems

## Platform Support

| Platform       | Filesystem Restrictions | Network Restrictions |
| -------------- | ----------------------- | -------------------- |
| Linux 6.7+     | ✅                      | ✅                   |
| Linux 5.13-6.6 | ✅                      | ❌                   |
| Linux < 5.13   | ❌                      | ❌                   |
| macOS          | ❌                      | ❌                   |
| Windows        | ❌                      | ❌                   |

On unsupported platforms, security configurations are ignored without errors, allowing the same configuration to work across different systems.

## Audit Mode

Audit mode allows you to see what file and network access your tasks would perform without applying restrictions. This is essential for developing and debugging security configurations.

### Enabling Audit Mode

```bash
# Run with global audit mode
cuenv task build --audit

# Run specific task with audit mode
cuenv task test --audit

# Set environment variable for all commands
export CUENV_AUDIT=1
cuenv task deploy
```

### Understanding Audit Output

Audit mode shows what would be blocked with security restrictions:

```bash
$ cuenv task build --audit
→ Executing task 'build'
[AUDIT] File access: READ /etc/passwd (would be DENIED)
[AUDIT] File access: WRITE /home/user/project/dist/app.js (would be ALLOWED)
[AUDIT] Network access: CONNECT api.example.com:443 (would be ALLOWED)
[AUDIT] Network access: CONNECT malicious.com:80 (would be DENIED)
→ Task completed successfully
```

### Using Audit Mode for Security Development

1. **Run tasks in audit mode** to see all access patterns:

```bash
cuenv task build --audit 2>&1 | grep AUDIT
```

2. **Analyze the output** to understand what access is needed:

```bash
# Save audit log for analysis
cuenv task build --audit 2> audit.log

# Extract file accesses
grep "File access" audit.log | sort | uniq

# Extract network accesses  
grep "Network access" audit.log | sort | uniq
```

3. **Create security configuration** based on audit results:

```cue
tasks: {
    "build": {
        command: "npm run build"
        security: {
            restrictDisk: true
            restrictNetwork: true
            readOnlyPaths: [
                "/usr", "/lib", "/bin",
                "./src", "./package.json", "./node_modules"
            ]
            readWritePaths: [
                "./dist", "/tmp"
            ]
            allowedHosts: ["registry.npmjs.org", "api.example.com"]
        }
    }
}
```

4. **Test the configuration** by running without audit mode:

```bash
# This should work with your new security config
cuenv task build
```

### Audit Mode in CI/CD

Use audit mode in CI to monitor for unexpected access patterns:

```bash
#!/bin/bash
# ci-security-check.sh

# Run build with audit logging
cuenv task build --audit 2> build-audit.log

# Check for unexpected network access
if grep -q "CONNECT.*suspicious-domain" build-audit.log; then
    echo "ERROR: Unexpected network access detected"
    exit 1
fi

# Check for unexpected file access outside project
if grep -q "READ /home/" build-audit.log; then
    echo "WARNING: Task accessing files outside project"
fi
```

## Inferred Security Capabilities

cuenv can automatically infer security restrictions based on task inputs and outputs, reducing the need for manual configuration.

### Automatic Path Inference

When using `inferFromInputsOutputs: true`, cuenv automatically allows access to:

- All paths specified in `inputs` (read-only)
- All paths specified in `outputs` (read-write)
- Standard system paths (`/usr`, `/lib`, `/bin`, etc.)
- Temporary directories (`/tmp`)

```cue
tasks: {
    "process-data": {
        command: "python process.py"
        inputs: [
            "./data/*.csv",
            "./scripts/process.py"
        ]
        outputs: [
            "./results/"
        ]
        security: {
            inferFromInputsOutputs: true
            restrictNetwork: true
        }
        // Automatically allows:
        // - READ access to ./data/*.csv and ./scripts/process.py
        // - WRITE access to ./results/
        // - Standard system paths for Python runtime
        // - Blocks all network access
    }
}
```

### Command-Based Inference

cuenv can infer capabilities based on the command being executed:

```cue
tasks: {
    "terraform-plan": {
        command: "terraform plan"
        security: {
            inferFromCommand: true
        }
        // Automatically infers:
        // - READ access to *.tf files
        // - WRITE access to .terraform/ directory
        // - Network access to Terraform providers
    }
}
```

### Manual Override with Inference

Combine automatic inference with manual overrides:

```cue
tasks: {
    "secure-build": {
        command: "npm run build"
        inputs: ["./src/**/*", "./package.json"]
        outputs: ["./dist/"]
        security: {
            inferFromInputsOutputs: true
            // Additional manual restrictions
            denyPaths: ["/etc/passwd", "/home"]
            allowedHosts: ["registry.npmjs.org"]
            // Block access to package.json modifications
            // (inferred as read-only from inputs)
        }
    }
}
```

### Capability-Based Security

Integrate with cuenv's capability system for dynamic security:

```cue
package env

capabilities: {
    aws: {
        commands: ["terraform", "aws"]
        security: {
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "./tf-files"]
            readWritePaths: ["./terraform-state"]
            allowedHosts: [
                "*.amazonaws.com",
                "registry.terraform.io"
            ]
        }
    }
    
    local: {
        commands: ["npm", "node"]
        security: {
            restrictDisk: true
            restrictNetwork: true
            readOnlyPaths: ["/usr", "/lib", "./src"]
            readWritePaths: ["./dist", "/tmp"]
        }
    }
}

tasks: {
    "deploy": {
        command: "terraform apply"
        // Automatically gets 'aws' capability security settings
    }
    
    "build": {
        command: "npm run build"
        // Automatically gets 'local' capability security settings
    }
}
```

### Debugging Inferred Security

Use audit mode to see what cuenv inferred:

```bash
$ cuenv task process-data --audit
[AUDIT] Inferred security configuration:
[AUDIT]   restrictDisk: true
[AUDIT]   readOnlyPaths: ["/usr", "/lib", "/bin", "./data", "./scripts/process.py"]
[AUDIT]   readWritePaths: ["./results", "/tmp"]
[AUDIT]   restrictNetwork: true
[AUDIT] Starting task execution...
```

View the complete inferred configuration:

```bash
# Show full inferred security config
RUST_LOG=debug cuenv task process-data --audit 2>&1 | grep "security config"
```

## Best Practices for Security

### 1. Always Test with Audit Mode First

```bash
# Development workflow
cuenv task new-feature --audit    # See what access is needed
# Add security config based on audit output
cuenv task new-feature            # Test with restrictions
```

### 2. Use Inference as a Starting Point

```cue
tasks: {
    "analyze": {
        command: "python analyze.py"
        inputs: ["./data/"]
        outputs: ["./reports/"]
        security: {
            inferFromInputsOutputs: true
            // Add specific restrictions as needed
            restrictNetwork: true
            denyPaths: ["/etc", "/home"]
        }
    }
}
```

### 3. Document Security Decisions

```cue
tasks: {
    "secure-process": {
        description: "Process sensitive data with restricted access"
        command: "python process.py"
        security: {
            restrictDisk: true
            restrictNetwork: true
            readOnlyPaths: ["./input-data"]
            readWritePaths: ["./output-data"]
            // Network blocked: no external data should be accessed
            // Filesystem restricted: only project data accessible
        }
    }
}
```

### 4. Regular Security Audits

```bash
#!/bin/bash
# audit-all-tasks.sh

for task in $(cuenv task | grep "^  " | awk '{print $1}'); do
    echo "=== Auditing task: $task ==="
    cuenv task "$task" --audit 2>&1 | grep AUDIT
    echo
done
```
