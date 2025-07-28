#!/usr/bin/env bash

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Build cuenv first
echo -e "${YELLOW}Building cuenv...${NC}"
cargo build || {
    echo -e "${RED}Failed to build cuenv${NC}"
    exit 1
}

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CUENV="$PROJECT_ROOT/target/debug/cuenv"
EXAMPLES_DIR="$PROJECT_ROOT/examples"

# Function to test a CUE directory
test_cue_dir() {
    local dir=$1
    local name=$(basename "$dir")
    
    echo -e "\n${YELLOW}Testing $name...${NC}"
    
    # Test basic load
    echo -n "  Load test: "
    if $CUENV load -d "$dir" > /dev/null 2>&1; then
        echo -e "${GREEN}PASS${NC}"
    else
        echo -e "${RED}FAIL${NC}"
        $CUENV load -d "$dir" 2>&1 | sed 's/^/    /'
        return 1
    fi
    
    # Test with environment if the file has environment configs
    if grep -q "environment:" "$dir/env.cue" 2>/dev/null; then
        echo -n "  Environment test (production): "
        if $CUENV load -d "$dir" -e production > /dev/null 2>&1; then
            echo -e "${GREEN}PASS${NC}"
        else
            echo -e "${RED}FAIL${NC}"
            return 1
        fi
    fi
    
    # Test with capabilities if the file has capability tags
    if grep -q "@capability" "$dir/env.cue" 2>/dev/null; then
        echo -n "  Capability test (aws): "
        if $CUENV load -d "$dir" -c aws > /dev/null 2>&1; then
            echo -e "${GREEN}PASS${NC}"
        else
            echo -e "${RED}FAIL${NC}"
            return 1
        fi
    fi
    
    return 0
}

# Test secret resolution with echo command
test_secret_resolution() {
    echo -e "\n${YELLOW}Testing secret resolution...${NC}"
    
    # Create a temporary test file
    TEMP_DIR=$(mktemp -d)
    cat > "$TEMP_DIR/env.cue" << 'EOF'
package env

env: {
    NORMAL_VAR: "plain-value"
    SECRET_VAR: {
        resolver: {
            command: "echo"
            args: ["resolved-secret"]
        }
    }
}
EOF

    echo -n "  Secret resolution test: "
    if OUTPUT=$(cd "$TEMP_DIR" && $CUENV exec env 2>&1); then
        if echo "$OUTPUT" | grep -q "SECRET_VAR=\\*\\*\\*\\*\\*\\*\\*\\*\\*\\*\\*"; then
            echo -e "${GREEN}PASS${NC} (secret properly masked)"
        else
            echo -e "${RED}FAIL - Secret not properly masked${NC}"
            echo "$OUTPUT" | sed 's/^/    /'
            return 1
        fi
    else
        echo -e "${RED}FAIL${NC}"
        echo "$OUTPUT" | sed 's/^/    /'
        return 1
    fi
    rm -rf "$TEMP_DIR"
}

# Run all tests
echo -e "${YELLOW}Running cuenv example tests${NC}"

# Initialize CUE module at examples level if it doesn't exist
if [ ! -f "$EXAMPLES_DIR/cue.mod/module.cue" ]; then
    echo -e "${YELLOW}Initializing CUE module in examples directory...${NC}"
    (cd "$EXAMPLES_DIR" && cue mod init github.com/rawkode/cuenv-examples) > /dev/null 2>&1 || {
        echo -e "${RED}Failed to initialize CUE module${NC}"
        exit 1
    }
fi

# Fetch dependencies (skip in Nix sandbox)
if [ -z "${CUENV_SKIP_CUE_FETCH:-}" ]; then
    echo -e "${YELLOW}Fetching CUE dependencies...${NC}"
    cd "$EXAMPLES_DIR"
    if ! cue mod tidy 2>&1; then
        echo -e "${RED}Failed to fetch CUE dependencies${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}Skipping CUE dependency fetch (running in sandbox)${NC}"
fi

FAILED=0

# Test each example directory
for example_dir in "$EXAMPLES_DIR"/*/; do
    if [ -d "$example_dir" ] && [ -f "$example_dir/env.cue" ]; then
        if ! test_cue_dir "$example_dir"; then
            FAILED=$((FAILED + 1))
        fi
    fi
done

# Test secret resolution
if ! test_secret_resolution; then
    FAILED=$((FAILED + 1))
fi

# Summary
echo -e "\n${YELLOW}Test Summary:${NC}"
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}$FAILED tests failed${NC}"
    exit 1
fi