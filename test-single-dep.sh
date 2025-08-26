#!/usr/bin/env bash
set -euo pipefail

# Integration test for single dependency update functionality
# Tests only things that can't be tested via Rust unit tests

echo "Testing single dependency update integration..."

# Save current directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Go to example directory
cd "$SCRIPT_DIR/example"

# First, run a full update to ensure we have a baseline
echo "1. Running full update to create baseline..."
nix develop ..# -c ../target/debug/uptix

# Test that the ergonomic patterns work correctly
echo "2. Testing ergonomic dependency patterns..."

# Test owner/repo pattern for GitHub release
if nix develop ..# -c ../target/debug/uptix --dependency "luizribeiro/hello-world-rs" 2>&1 | grep "Found 1 dependencies matching" > /dev/null; then
    echo "✓ owner/repo pattern works for GitHub release"
else
    echo "ERROR: owner/repo pattern failed"
    exit 1
fi

# Test owner/repo:branch pattern for GitHub branch
if nix develop ..# -c ../target/debug/uptix --dependency "luizribeiro/hello-world-rs:main" 2>&1 | grep "Found 1 dependencies matching" > /dev/null; then
    echo "✓ owner/repo:branch pattern works for GitHub branch"
else
    echo "ERROR: owner/repo:branch pattern failed"
    exit 1
fi

# Test that lock file preserves other entries during single update
echo "3. Testing lock file preservation..."
BEFORE_COUNT=$(jq 'keys | length' uptix.lock)
nix develop ..# -c ../target/debug/uptix --dependency "postgres:15"
AFTER_COUNT=$(jq 'keys | length' uptix.lock)

if [ "$BEFORE_COUNT" != "$AFTER_COUNT" ]; then
    echo "ERROR: Lock file entry count changed! Before: $BEFORE_COUNT, After: $AFTER_COUNT"
    exit 1
fi
echo "✓ Lock file preserves all entries during single update"

echo "All integration tests passed!"

# Restore the lock file
nix develop ..# -c ../target/debug/uptix