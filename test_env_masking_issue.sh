#!/bin/bash

# Create a test directory
TEST_DIR=$(mktemp -d)
cd "$TEST_DIR"

# Create a CUE file with a secret
cat > env.cue << 'EOF'
package env

env: {
    NORMAL_VAR: "hello"
    SECRET_VAR: "mysecretvalue"
}
EOF

# First, let's see what the raw env command outputs
echo "=== Testing raw env command ==="
SECRET_VAR=mysecretvalue env | grep SECRET_VAR

# Now test with cuenv
echo -e "\n=== Testing cuenv run env ==="
~/Code/src/github.com/rawkode/cuenv/target/debug/cuenv run env | grep -E "(SECRET_VAR|NORMAL_VAR)"

# Clean up
cd /
rm -rf "$TEST_DIR"