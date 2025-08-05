#\!/bin/bash
set -e

# Get nix output
echo "Getting nix print-dev-env output..."
NIX_OUTPUT=$(nix print-dev-env)
echo "Got ${#NIX_OUTPUT} bytes of output"

# Create temp script to evaluate 
TEMP_SCRIPT=$(mktemp)
cat > "$TEMP_SCRIPT" << 'SCRIPT_EOF'
#\!/bin/bash
set -e
# Source the nix environment
SCRIPT_OUTPUT_VAR
# Print PATH specifically
echo "EVALUATED_PATH=$PATH"
env -0
SCRIPT_EOF

# Replace the placeholder with actual nix output, escaping it properly
ESCAPED_NIX_OUTPUT=$(printf '%s\n' "$NIX_OUTPUT" | sed 's/[[\.*^$()+?{|]/\\&/g')
sed -i "s/SCRIPT_OUTPUT_VAR/$ESCAPED_NIX_OUTPUT/g" "$TEMP_SCRIPT"

chmod +x "$TEMP_SCRIPT"

echo "Executing evaluation script..."
OUTPUT=$("$TEMP_SCRIPT")

echo "PATH from evaluated environment:"
echo "$OUTPUT" | grep "EVALUATED_PATH=" | cut -d= -f2 | tr ':' '\n' | grep treefmt || echo "treefmt not found in PATH"

rm -f "$TEMP_SCRIPT"
