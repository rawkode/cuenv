#!/usr/bin/env bash
# Setup script for Bazel integration with cuenv

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🚀 Setting up Bazel build system for cuenv"

# Check if running in nix develop
if [ -z "${IN_NIX_DEVELOP:-}" ]; then
    echo "⚠️  Not running in nix develop environment"
    echo "Run: nix develop -c \"$0 $@\""
    exit 1
fi

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
echo "Checking prerequisites..."
if ! command_exists bazel; then
    echo "❌ Bazel not found. Please install Bazel or update your nix flake"
    exit 1
fi

if ! command_exists cargo; then
    echo "❌ Cargo not found. Please install Rust"
    exit 1
fi

echo "✅ All prerequisites met"

# Build the remote cache server
echo "Building remote cache server..."
cd "$PROJECT_ROOT"
cargo build --release --bin remote_cache_server

# Create cache directory
CACHE_DIR="${CUENV_CACHE_DIR:-/tmp/cuenv-cache}"
echo "Creating cache directory at $CACHE_DIR..."
mkdir -p "$CACHE_DIR"

# Start remote cache server in background
echo "Starting remote cache server..."
CACHE_PID_FILE="/tmp/cuenv-remote-cache.pid"
if [ -f "$CACHE_PID_FILE" ]; then
    OLD_PID=$(cat "$CACHE_PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        echo "Remote cache server already running (PID: $OLD_PID)"
    else
        rm "$CACHE_PID_FILE"
    fi
fi

if [ ! -f "$CACHE_PID_FILE" ]; then
    "$PROJECT_ROOT/target/release/remote_cache_server" \
        --address 127.0.0.1:50051 \
        --cache-dir "$CACHE_DIR" \
        --log-level info \
        > /tmp/cuenv-remote-cache.log 2>&1 &
    
    CACHE_PID=$!
    echo $CACHE_PID > "$CACHE_PID_FILE"
    echo "Remote cache server started (PID: $CACHE_PID)"
    echo "Logs: /tmp/cuenv-remote-cache.log"
    
    # Wait for server to be ready
    echo "Waiting for server to be ready..."
    for i in {1..30}; do
        if nc -z 127.0.0.1 50051 2>/dev/null; then
            echo "✅ Remote cache server is ready"
            break
        fi
        sleep 1
    done
fi

# Run initial Bazel build
echo "Running initial Bazel build..."
cd "$PROJECT_ROOT"
if bazel build //:cuenv --config=remote; then
    echo "✅ Bazel build successful!"
else
    echo "❌ Bazel build failed"
    echo "Check logs at /tmp/cuenv-remote-cache.log"
    exit 1
fi

# Print helpful commands
cat << EOF

🎉 Bazel setup complete!

Useful commands:
  Build:     bazel build //:cuenv --config=remote
  Test:      bazel test //... --config=remote
  Clean:     bazel clean
  
Remote cache server:
  Status:    ps -p \$(cat $CACHE_PID_FILE)
  Logs:      tail -f /tmp/cuenv-remote-cache.log
  Stop:      kill \$(cat $CACHE_PID_FILE)
  
Build profiles:
  Debug:     bazel build //:cuenv --config=debug
  Release:   bazel build //:cuenv --config=release
  
For more information, see: docs/bazel-migration.md
EOF