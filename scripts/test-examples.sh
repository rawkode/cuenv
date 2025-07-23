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

# Function to test a CUE file
test_cue_file() {
    local file=$1
    local name=$(basename "$file")
    
    echo -e "\n${YELLOW}Testing $name...${NC}"
    
    # Test basic load
    echo -n "  Load test: "
    if CUENV_FILE="$name" $CUENV load -d "$EXAMPLES_DIR" > /dev/null 2>&1; then
        echo -e "${GREEN}PASS${NC}"
    else
        echo -e "${RED}FAIL${NC}"
        CUENV_FILE="$name" $CUENV load -d "$EXAMPLES_DIR" 2>&1 | sed 's/^/    /'
        return 1
    fi
    
    # Test with environment if the file has environment configs
    if grep -q "environment:" "$file"; then
        echo -n "  Environment test (production): "
        if CUENV_FILE="$name" $CUENV load -d "$EXAMPLES_DIR" -e production > /dev/null 2>&1; then
            echo -e "${GREEN}PASS${NC}"
        else
            echo -e "${RED}FAIL${NC}"
            return 1
        fi
    fi
    
    # Test with capabilities if the file has capability tags
    if grep -q "@capability" "$file"; then
        echo -n "  Capability test (aws): "
        if CUENV_FILE="$name" $CUENV load -d "$EXAMPLES_DIR" -c aws > /dev/null 2>&1; then
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

#SecretRef: {
    resolver: {
        cmd: string
        args: [...string]
    }
    ...
}

#EchoSecret: #SecretRef & {
    value: string
    resolver: {
        cmd: "echo"
        args: [value]
    }
}

NORMAL_VAR: "plain-value"
SECRET_VAR: #EchoSecret & { value: "resolved-secret" }
EOF

    echo -n "  Secret resolution test: "
    if OUTPUT=$(cd "$TEMP_DIR" && $CUENV run env 2>&1); then
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

FAILED=0

# Test each example file
for cue_file in "$EXAMPLES_DIR"/*.cue; do
    if ! test_cue_file "$cue_file"; then
        FAILED=$((FAILED + 1))
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