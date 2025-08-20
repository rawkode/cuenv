#!/usr/bin/env bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Background Source Hook Test ===${NC}"
echo "Testing background hooks that export environment variables"
echo ""

# Get the directory of this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
CUENV_BIN="${SCRIPT_DIR}/../../target/debug/cuenv"

# Build if needed
if [ ! -f "$CUENV_BIN" ]; then
    echo -e "${YELLOW}Building cuenv...${NC}"
    (cd "${SCRIPT_DIR}/../.." && nix develop -c cargo build)
fi

cd "$SCRIPT_DIR"

echo -e "${GREEN}Step 1: Allow the directory${NC}"
"$CUENV_BIN" env allow . || true

echo -e "${GREEN}Step 2: Start hooks (they run automatically in background)${NC}"
# Hooks now run in background by default for shell operations
"$CUENV_BIN" env allow . 2>&1 | grep -E "(Running|background)" || true

echo ""
echo -e "${GREEN}Step 3: Check hook status while running${NC}"
sleep 1
"$CUENV_BIN" env status --hooks

echo ""
echo -e "${GREEN}Step 4: Wait for hooks to complete (5 seconds)${NC}"
for i in {1..5}; do
    echo -n "."
    sleep 1
done
echo ""

echo ""
echo -e "${GREEN}Step 5: Check hook status after completion${NC}"
"$CUENV_BIN" env status --hooks

echo ""
echo -e "${GREEN}Step 6: Simulate shell hook to capture environment${NC}"
SHELL_OUTPUT=$("$CUENV_BIN" shell hook bash 2>/dev/null)
if echo "$SHELL_OUTPUT" | grep -q "TEST_BG_VAR"; then
    echo -e "${GREEN}✓ Environment captured successfully!${NC}"
    echo "$SHELL_OUTPUT" | grep "TEST_BG_VAR"
    echo "$SHELL_OUTPUT" | grep "TEST_TIMESTAMP" || true
else
    echo -e "${RED}✗ No environment captured${NC}"
    echo "Shell output: $SHELL_OUTPUT"
fi

echo ""
echo -e "${GREEN}Step 7: Verify the captured environment was cleared${NC}"
SECOND_OUTPUT=$("$CUENV_BIN" shell hook bash 2>/dev/null)
if echo "$SECOND_OUTPUT" | grep -q "TEST_BG_VAR"; then
    echo -e "${RED}✗ Environment was sourced again (should have been cleared)${NC}"
else
    echo -e "${GREEN}✓ Captured environment was properly cleared${NC}"
fi

echo ""
echo -e "${GREEN}Test complete!${NC}"