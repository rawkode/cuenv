# Background Source Hooks Example

This example demonstrates how cuenv handles long-running source hooks that run automatically in the background and provide environment variables to the shell.

## Features Demonstrated

1. **Source hooks** - Hooks marked with `source: true` that export environment variables
2. **Automatic background execution** - Shell hooks run in background by default
3. **Automatic environment capture** - Shell hook detects completed background hooks
4. **One-time sourcing** - Captured environment is only sourced once

## How It Works

1. The `env.cue` defines a hook that:
   - Sleeps for 5 seconds (simulating a long-running task)
   - Exports `TEST_BG_VAR` and `TEST_TIMESTAMP` environment variables
   - Is marked as `source: true` to capture the output

2. When you enter a directory with cuenv shell integration:
   - The hook starts executing automatically in the background
   - You can continue working while the hook runs
   - The hook continues running without blocking the shell

3. The shell integration (`cuenv shell hook`):
   - Checks for completed background hooks on each prompt
   - Sources any captured environment variables
   - Clears the capture file to prevent re-sourcing

## Running the Test

### Automated Test

```bash
bash test.sh
```

### Manual Test

```bash
# 1. Allow the directory
../../target/debug/cuenv env allow .

# 2. The hooks will start running automatically in the background

# 3. Check status while running
../../target/debug/cuenv env status

# 4. Wait 5 seconds for hooks to complete

# 5. Run shell hook to capture environment
../../target/debug/cuenv shell hook bash

# You should see:
# export TEST_BG_VAR="background_hook_completed"
# export TEST_TIMESTAMP="<timestamp>"
```

## Expected Output

The test should show:

1. Hooks starting automatically in the background
2. Status showing 1 running hook
3. After 5 seconds, status showing 0 hooks
4. Shell hook outputting the captured environment variables
5. Second shell hook NOT outputting anything (properly cleared)

## Use Cases

This pattern is useful for:

- Slow initialization scripts (e.g., downloading dependencies)
- Authentication tokens that take time to fetch
- Database connection strings from vault/secret managers
- Any environment setup that shouldn't block the shell
