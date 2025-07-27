#!/usr/bin/env bash
# Quick test script to verify Bazel configuration

set -euo pipefail

echo "Testing Bazel configuration..."

# Check if WORKSPACE.bazel exists
if [ -f "WORKSPACE.bazel" ]; then
    echo "✅ WORKSPACE.bazel found"
else
    echo "❌ WORKSPACE.bazel not found"
    exit 1
fi

# Check if BUILD.bazel exists
if [ -f "BUILD.bazel" ]; then
    echo "✅ BUILD.bazel found"
else
    echo "❌ BUILD.bazel not found"
    exit 1
fi

# Check if .bazelrc exists
if [ -f ".bazelrc" ]; then
    echo "✅ .bazelrc found"
else
    echo "❌ .bazelrc not found"
    exit 1
fi

# Check remote cache module
if [ -d "src/remote_cache" ]; then
    echo "✅ Remote cache module found"
    if [ -f "src/remote_cache/server.rs" ]; then
        echo "  ✅ server.rs found"
    fi
    if [ -f "src/remote_cache/bin/server.rs" ]; then
        echo "  ✅ bin/server.rs found"
    fi
    if [ -f "src/remote_cache/BUILD.bazel" ]; then
        echo "  ✅ BUILD.bazel found"
    fi
else
    echo "❌ Remote cache module not found"
fi

# Check documentation
if [ -f "docs/bazel-migration.md" ]; then
    echo "✅ Bazel migration documentation found"
else
    echo "❌ Bazel migration documentation not found"
fi

echo ""
echo "Bazel configuration test complete!"
echo ""
echo "Next steps:"
echo "1. Run: nix develop"
echo "2. Run: ./scripts/bazel-setup.sh"
echo "3. Build with: bazel build //..."