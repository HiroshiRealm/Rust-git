#!/bin/bash

# This script performs an end-to-end test of the HTTP fetch/pull/push functionality.
# It will:
# 1. Build the project.
# 2. Set up a temporary directory with a "server" and "client" repository.
# 3. Start the HTTP server in the background.
# 4. Run `pull` on the client and verify the result.
# 5. Run `push` from the client and verify the result on the server.
# 6. Clean up all temporary files and processes.

# Exit immediately if a command exits with a non-zero status.
set -e

echo "--- Rust-Git HTTP End-to-End Test ---"

# --- Configuration ---
# Assuming the script is run from the project root
PROJECT_ROOT=$(cd -- "$(dirname -- "$0")/../.." &> /dev/null && pwd)
RUST_GIT_BIN="$PROJECT_ROOT/target/debug/rust-git"
SERVER_BIN="$PROJECT_ROOT/target/debug/server"
TEST_DIR="/tmp/rust-git-test"
SERVER_URL="http://127.0.0.1:3000/repo.bundle"

# --- Cleanup function ---
# This function is called on script exit to ensure cleanup happens.
cleanup() {
  echo "--- Cleaning up ---"
  # The SERVER_PID variable is global
  if [ ! -z "$SERVER_PID" ]; then
    echo "Stopping server (PID: $SERVER_PID)..."
    # Kill the process, || true ignores errors if it's already gone
    kill $SERVER_PID || true
  fi
  echo "Removing test directory: $TEST_DIR"
  rm -rf "$TEST_DIR"
}

# Trap the EXIT signal to run the cleanup function automatically
trap cleanup EXIT

# --- 1. Build binaries ---
echo "--- Building project ---"
cargo build

# --- 2. Setup test environment ---
echo "--- Setting up test environment in $TEST_DIR ---"
mkdir -p "$TEST_DIR/server_repo"
mkdir -p "$TEST_DIR/client_repo"

# --- 3. Initialize Server Repo ---
echo "--- Initializing server repository ---"
cd "$TEST_DIR/server_repo"
"$RUST_GIT_BIN" init
echo "Hello from the server!" > server_file.txt
"$RUST_GIT_BIN" add server_file.txt
"$RUST_GIT_BIN" commit -m "Initial server commit"

# --- 4. Start Server ---
echo "--- Starting HTTP server ---"
# Start the server in the background
"$SERVER_BIN" "$TEST_DIR/server_repo" &
# Save its Process ID (PID)
SERVER_PID=$!
echo "Server started with PID: $SERVER_PID"
# Give the server a moment to start up and bind to the port
sleep 2

# --- 5. Test Pull ---
echo "--- Testing client pull ---"
cd "$TEST_DIR/client_repo"
"$RUST_GIT_BIN" init
"$RUST_GIT_BIN" pull origin "$SERVER_URL"

# Verify pull
echo "Verifying pull..."
if [ -f "server_file.txt" ] && [ "$(cat server_file.txt)" = "Hello from the server!" ]; then
    echo "✅ Pull successful: server_file.txt found with correct content."
else
    echo "❌ Pull FAILED: server_file.txt not found or content mismatch."
    exit 1
fi

# --- 6. Test Push ---
echo "--- Testing client push ---"
echo "A new file from the client." > client_file.txt
"$RUST_GIT_BIN" add client_file.txt
"$RUST_GIT_BIN" commit -m "Commit from client"
# Get the commit hash that we just created
CLIENT_COMMIT_HASH=$(cat .git/refs/heads/master)
echo "Client is at commit: $CLIENT_COMMIT_HASH"

"$RUST_GIT_BIN" push origin "$SERVER_URL"

# --- 7. Verify Push ---
echo "Verifying push..."
# The server process must be stopped to safely read its git files,
# as it doesn't automatically reload them.
echo "Stopping server to check result..."
kill $SERVER_PID
# Clear the variable so cleanup doesn't try to kill it again
SERVER_PID=""
# Wait a moment to ensure the server has released file locks
sleep 1

SERVER_COMMIT_HASH=$(cat "$TEST_DIR/server_repo/.git/refs/heads/master")
echo "Server is now at commit: $SERVER_COMMIT_HASH"

if [ "$CLIENT_COMMIT_HASH" = "$SERVER_COMMIT_HASH" ]; then
    echo "✅ Push successful: Server's master branch was updated correctly."
else
    echo "❌ Push FAILED: Server's master branch was not updated."
    exit 1
fi

echo ""
echo "🎉 All tests passed successfully! 🎉"

# The 'trap' will handle the final cleanup.
exit 0 