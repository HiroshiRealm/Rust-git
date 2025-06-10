#!/bin/bash

# This script performs an end-to-end test specifically for using remote aliases
# (`origin`) with the fetch, pull, and push commands.

set -e

echo "--- Rust-Git Remote Alias Test ---"

# --- Configuration ---
PROJECT_ROOT=$(cd -- "$(dirname -- "$0")/../.." &> /dev/null && pwd)
RUST_GIT_BIN="$PROJECT_ROOT/target/debug/rust-git"
SERVER_BIN="$PROJECT_ROOT/target/debug/server"
TEST_DIR="/tmp/rust-git-alias-test"
SERVER_URL="http://127.0.0.1:3000/repo.bundle"

# --- Cleanup function ---
cleanup() {
  echo "--- Cleaning up ---"
  if [ ! -z "$SERVER_PID" ]; then
    echo "Stopping server (PID: $SERVER_PID)..."
    kill $SERVER_PID || true
  fi
  echo "Removing test directory: $TEST_DIR"
  rm -rf "$TEST_DIR"
}

trap cleanup EXIT

# --- 1. Build & Setup ---
echo "--- Building project ---"
cargo build

echo "--- Setting up test environment in $TEST_DIR ---"
rm -rf "$TEST_DIR" # Clean any previous runs
mkdir -p "$TEST_DIR/server_repo"
mkdir -p "$TEST_DIR/client_repo"

# --- 2. Initialize Server & Start ---
echo "--- Initializing server repository ---"
cd "$TEST_DIR/server_repo"
"$RUST_GIT_BIN" init
echo "Hello from the aliased remote!" > server_file.txt
"$RUST_GIT_BIN" add server_file.txt
"$RUST_GIT_BIN" commit -m "Initial server commit"

echo "--- Starting HTTP server ---"
"$SERVER_BIN" "$TEST_DIR/server_repo" &
SERVER_PID=$!
echo "Server started with PID: $SERVER_PID"
sleep 2

# --- 3. Client sets up remote and pulls ---
echo "--- Client setting up remote 'origin' and pulling... ---"
cd "$TEST_DIR/client_repo"
"$RUST_GIT_BIN" init
"$RUST_GIT_BIN" remote add origin "$SERVER_URL"
"$RUST_GIT_BIN" pull origin

# Verify pull
echo "Verifying pull via alias..."
if [ -f "server_file.txt" ] && [ "$(cat server_file.txt)" = "Hello from the aliased remote!" ]; then
    echo "✅ Pull via alias successful."
else
    echo "❌ Pull via alias FAILED."
    exit 1
fi

# --- 4. Client fetches and merges manually ---
echo "--- Client testing fetch and merge via alias... ---"
# First, create a new commit on the server to make the client outdated
cd "$TEST_DIR/server_repo"
echo "A second file from server" > server_file_2.txt
"$RUST_GIT_BIN" add .
"$RUST_GIT_BIN" commit -m "Second server commit"

# Now, client fetches
cd "$TEST_DIR/client_repo"
"$RUST_GIT_BIN" fetch origin
echo "✅ Fetch via alias completed."

# Verify that the remote-tracking branch is updated, but the local branch is not
if [ -f "server_file_2.txt" ]; then
    echo "❌ Fetch FAILED: Working directory was modified by fetch."
    exit 1
fi
echo "Verifying merge..."
"$RUST_GIT_BIN" merge origin/master

if [ ! -f "server_file_2.txt" ]; then
    echo "❌ Merge FAILED: server_file_2.txt was not created after merge."
    exit 1
fi
echo "✅ Manual merge of fetched branch successful."

# --- 5. Client pushes using alias ---
echo "--- Client pushing a new commit via alias... ---"
cd "$TEST_DIR/client_repo"
echo "A new file from the client." > client_file.txt
"$RUST_GIT_BIN" add client_file.txt
"$RUST_GIT_BIN" commit -m "Commit from client"
"$RUST_GIT_BIN" push origin

# --- 6. Final Verification ---
echo "Verifying push via alias..."
kill $SERVER_PID
SERVER_PID="" # Prevent cleanup from trying to kill again
sleep 1

cd "$TEST_DIR/server_repo"
# Use checkout to update the working directory to the latest master
"$RUST_GIT_BIN" checkout master

if [ -f "client_file.txt" ]; then
    echo "✅ Push via alias successful: client_file.txt is present on the server."
else
    echo "❌ Push via alias FAILED: client_file.txt not found on the server."
    ls -l
    exit 1
fi

echo ""
echo "🎉 All remote alias tests passed successfully! 🎉"

exit 0 