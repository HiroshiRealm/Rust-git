#!/bin/bash

# This script no longer functions well because we have changed from path to url. 

# This script performs an end-to-end test of the local bundle-based
# fetch/pull/push functionality.

set -e

echo "--- Rust-Git Local Bundle End-to-End Test ---"

# --- Configuration ---
# Find project root regardless of script execution location
PROJECT_ROOT=$(cd -- "$(dirname -- "$0")/../.." &> /dev/null && pwd)
RUST_GIT_BIN="$PROJECT_ROOT/target/debug/rust-git"
TEST_DIR="/tmp/rust-git-local-test"

# --- Cleanup function ---
cleanup() {
  echo "--- Cleaning up ---"
  echo "Removing test directory: $TEST_DIR"
  rm -rf "$TEST_DIR"
}

trap cleanup EXIT

# --- 1. Build & Setup ---
echo "--- Building project ---"
cargo build

echo "--- Setting up test environment in $TEST_DIR ---"
rm -rf "$TEST_DIR" # Clean previous runs
mkdir -p "$TEST_DIR/server_repo"
mkdir -p "$TEST_DIR/client_repo"
mkdir -p "$TEST_DIR/bundles"

# --- 2. Initialize Server Repo ---
echo "--- Initializing server repository ---"
cd "$TEST_DIR/server_repo"
"$RUST_GIT_BIN" init
echo "Hello from the server!" > server_file.txt
"$RUST_GIT_BIN" add server_file.txt
"$RUST_GIT_BIN" commit -m "Initial server commit"

# --- 3. Simulate a "clone" by creating a bundle and pulling it ---
echo "--- Simulating clone: Server pushes to bundle, client pulls from bundle ---"
"$RUST_GIT_BIN" push origin "$TEST_DIR/bundles/initial.bundle"

cd "$TEST_DIR/client_repo"
"$RUST_GIT_BIN" init
"$RUST_GIT_BIN" pull origin "$TEST_DIR/bundles/initial.bundle"

echo "Verifying clone/pull..."
if [ -f "server_file.txt" ] && [ "$(cat server_file.txt)" = "Hello from the server!" ]; then
    echo "✅ Pull successful: server_file.txt found with correct content."
else
    echo "❌ Pull FAILED: server_file.txt not found or content mismatch."
    exit 1
fi

# --- 4. Client pushes an update ---
echo "--- Client pushing an update ---"
cd "$TEST_DIR/client_repo"
echo "A new file from the client." > client_file.txt
"$RUST_GIT_BIN" add client_file.txt
"$RUST_GIT_BIN" commit -m "Commit from client"
"$RUST_GIT_BIN" push origin "$TEST_DIR/bundles/client_update.bundle"

# --- 5. Server pulls the update ---
echo "--- Server pulling the client's update ---"
cd "$TEST_DIR/server_repo"
"$RUST_GIT_BIN" pull origin "$TEST_DIR/bundles/client_update.bundle"

echo "Verifying server update..."
# Use checkout to update the working directory to the latest master
"$RUST_GIT_BIN" checkout master
if [ -f "client_file.txt" ] && [ "$(cat client_file.txt)" = "A new file from the client." ]; then
    echo "✅ Server update successful: client_file.txt found with correct content."
else
    echo "❌ Server update FAILED: client_file.txt not found or content mismatch."
    ls -l
    exit 1
fi

echo ""
echo "🎉 All local bundle tests passed successfully! 🎉"

exit 0