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
../target/debug/uptix update

# Test that the ergonomic patterns work correctly
echo "2. Testing ergonomic dependency patterns..."

# Test owner/repo pattern for GitHub release
if ../target/debug/uptix update --dependency "luizribeiro/hello-world-rs" 2>&1 | grep "Found 1 dependencies matching" > /dev/null; then
    echo "✓ owner/repo pattern works for GitHub release"
else
    echo "ERROR: owner/repo pattern failed"
    exit 1
fi

# Test owner/repo:branch pattern for GitHub branch
if ../target/debug/uptix update --dependency "luizribeiro/hello-world-rs:main" 2>&1 | grep "Found 1 dependencies matching" > /dev/null; then
    echo "✓ owner/repo:branch pattern works for GitHub branch"
else
    echo "ERROR: owner/repo:branch pattern failed"
    exit 1
fi

# Test that lock file preserves other entries during single update
echo "3. Testing lock file preservation..."
BEFORE_COUNT=$(jq 'keys | length' uptix.lock)
../target/debug/uptix update --dependency "postgres:15"
AFTER_COUNT=$(jq 'keys | length' uptix.lock)

if [ "$BEFORE_COUNT" != "$AFTER_COUNT" ]; then
    echo "ERROR: Lock file entry count changed! Before: $BEFORE_COUNT, After: $AFTER_COUNT"
    exit 1
fi
echo "✓ Lock file preserves all entries during single update"

echo "All integration tests passed!"

# Test new commands
echo "4. Testing new commands..."

# Test list command
echo "Testing 'uptix list'..."
if ../target/debug/uptix list | grep "Dependencies found in project" > /dev/null; then
    echo "✓ list command works"
else
    echo "ERROR: list command failed"
    exit 1
fi

# Test show command
echo "Testing 'uptix show'..."
if ../target/debug/uptix show "postgres:15" | grep "Dependency: postgres:15" > /dev/null; then
    echo "✓ show command works"
else
    echo "ERROR: show command failed"
    exit 1
fi

# Restore the lock file
../target/debug/uptix update