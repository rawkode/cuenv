#\!/bin/bash
set -e

# Get nix output and save to temp file
echo "Getting nix print-dev-env output..."
NIX_OUTPUT_FILE=$(mktemp)
nix print-dev-env > "$NIX_OUTPUT_FILE"
echo "Got $(wc -c < "$NIX_OUTPUT_FILE") bytes of output"

# Create evaluation script
EVAL_SCRIPT=$(mktemp)
cat > "$EVAL_SCRIPT" << 'SCRIPT_EOF'
#\!/bin/bash
set -e
# Source the nix environment
source REPLACE_WITH_NIX_FILE
# Print PATH specifically
echo "EVALUATED_PATH=$PATH"
SCRIPT_EOF

# Replace placeholder with actual file path
sed -i "s|REPLACE_WITH_NIX_FILE|$NIX_OUTPUT_FILE|g" "$EVAL_SCRIPT"
chmod +x "$EVAL_SCRIPT"

echo "Executing evaluation script..."
OUTPUT=$("$EVAL_SCRIPT" 2>&1) || echo "Script failed: $?"

echo "Output from evaluation:"
echo "$OUTPUT" | head -10

echo "Looking for PATH..."
echo "$OUTPUT" | grep "EVALUATED_PATH=" | cut -d= -f2 | tr ':' '\n' | grep treefmt || echo "treefmt not found in PATH"

# Cleanup
rm -f "$NIX_OUTPUT_FILE" "$EVAL_SCRIPT"
