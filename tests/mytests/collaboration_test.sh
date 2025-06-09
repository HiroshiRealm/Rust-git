#!/bin/bash

# This script simulates a collaboration scenario with two clients and one server
# to test the non-fast-forward push rejection feature.
#
# Workflow:
# 1. Server starts with an initial commit.
# 2. Client 1 clones the repo.
# 3. Client 2 clones the repo.
# 4. Client 1 pushes a new commit successfully.
# 5. Client 2, now out of date, tries to push its own commit and fails.
# 6. Client 2 pulls the changes, merges them, and successfully pushes again.

set -e

echo "--- Rust-Git Collaboration/Conflict Test ---"

# --- Configuration ---
PROJECT_ROOT=$(cd -- "$(dirname -- "$0")/../.." &> /dev/null && pwd)
RUST_GIT_BIN="$PROJECT_ROOT/target/debug/rust-git"
SERVER_BIN="$PROJECT_ROOT/target/debug/server"
TEST_DIR="/tmp/rust-git-collab-test"
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
mkdir -p "$TEST_DIR/client1_repo"
mkdir -p "$TEST_DIR/client2_repo"

# --- 2. Initialize Server & Start ---
echo "--- Initializing server repository ---"
cd "$TEST_DIR/server_repo"
"$RUST_GIT_BIN" init
echo "Initial file" > initial_file.txt
"$RUST_GIT_BIN" add initial_file.txt
"$RUST_GIT_BIN" commit -m "Initial commit"

echo "--- Starting HTTP server ---"
"$SERVER_BIN" "$TEST_DIR/server_repo" &
SERVER_PID=$!
echo "Server started with PID: $SERVER_PID"
sleep 2

# --- 3. Both clients clone the repo ---
echo "--- Client 1 cloning... ---"
cd "$TEST_DIR/client1_repo"
"$RUST_GIT_BIN" init
"$RUST_GIT_BIN" pull origin "$SERVER_URL"
echo "Client 1 cloned."

echo "--- Client 2 cloning... ---"
cd "$TEST_DIR/client2_repo"
"$RUST_GIT_BIN" init
"$RUST_GIT_BIN" pull origin "$SERVER_URL"
echo "Client 2 cloned."

# --- 4. Client 1 makes a change and pushes successfully ---
echo "--- Client 1 pushing a new commit... ---"
cd "$TEST_DIR/client1_repo"
echo "Change from client 1" > client1_file.txt
"$RUST_GIT_BIN" add client1_file.txt
"$RUST_GIT_BIN" commit -m "Client 1 commit"
"$RUST_GIT_BIN" push origin "$SERVER_URL"
echo "✅ Client 1 push successful."

# --- 5. Client 2 makes a change and fails to push ---
echo "--- Client 2 attempting a non-fast-forward push (this should fail)... ---"
cd "$TEST_DIR/client2_repo"
echo "Change from client 2" > client2_file.txt
"$RUST_GIT_BIN" add client2_file.txt
"$RUST_GIT_BIN" commit -m "Client 2 commit"

# We expect this command to fail, so we use '!' to invert the exit code.
# If it fails (non-zero exit code), '!' makes it success (zero exit code).
if ! "$RUST_GIT_BIN" push origin "$SERVER_URL" ; then
    echo "✅ Push correctly failed as non-fast-forward."
else
    echo "❌ FAILED: Push was accepted but should have been rejected."
    exit 1
fi

# --- 6. Client 2 pulls, merges, and pushes again ---
echo "--- Client 2 pulling to resolve conflict... ---"
"$RUST_GIT_BIN" pull origin "$SERVER_URL"
# Our merge logic is simple, but since files are different, it should work.

echo "--- Client 2 retrying push... ---"
"$RUST_GIT_BIN" push origin "$SERVER_URL"
echo "✅ Client 2 second push successful."

# --- 7. Final Verification ---
echo "--- Verifying final state on server ---"
kill $SERVER_PID
SERVER_PID="" # Prevent cleanup from trying to kill again
sleep 1

cd "$TEST_DIR/server_repo"
# To verify, we'll check out the master branch to update the working directory
"$RUST_GIT_BIN" checkout master

if [ -f "initial_file.txt" ] && [ -f "client1_file.txt" ] && [ -f "client2_file.txt" ]; then
    echo "✅ Verification successful: All three files are present on the server."
else
    echo "❌ Verification FAILED: Not all files were found on the server."
    ls -l
    exit 1
fi

echo ""
echo "🎉 Collaboration test passed successfully! 🎉"

exit 0 