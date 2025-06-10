#!/bin/bash

# This test simulates a common collaboration scenario where two clients
# interact with the same remote repository, testing non-fast-forward push rejection.

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
cargo build --quiet

echo "--- Setting up test environment in $TEST_DIR ---"
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR/server_repo"
mkdir -p "$TEST_DIR/client1_repo"
mkdir -p "$TEST_DIR/client2_repo"

# --- 2. Initialize Server & Start ---
echo "--- Initializing server repository ---"
cd "$TEST_DIR/server_repo"
"$RUST_GIT_BIN" init
echo "Hello from the server!" > common_file.txt
"$RUST_GIT_BIN" add common_file.txt
"$RUST_GIT_BIN" commit -m "Initial commit"

echo "--- Starting HTTP server ---"
# Start server in the background and redirect its output to a log file
"$SERVER_BIN" "$TEST_DIR/server_repo" &> /tmp/rust-git-server.log &
SERVER_PID=$!
echo "Server started with PID: $SERVER_PID. Log: /tmp/rust-git-server.log"
sleep 2

# --- 3. Both clients pull the initial state ---
echo "--- Both clients pulling initial state ---"
cd "$TEST_DIR/client1_repo"
"$RUST_GIT_BIN" init
"$RUST_GIT_BIN" remote add origin "$SERVER_URL"
"$RUST_GIT_BIN" pull origin
echo "✅ Client 1 pulled initial commit."

cd "$TEST_DIR/client2_repo"
"$RUST_GIT_BIN" init
"$RUST_GIT_BIN" remote add origin "$SERVER_URL"
"$RUST_GIT_BIN" pull origin
echo "✅ Client 2 pulled initial commit."

# --- 4. Client 1 pushes a change successfully ---
echo "--- Client 1 pushing a change... ---"
cd "$TEST_DIR/client1_repo"
echo "Change from client 1" >> client1_file.txt
"$RUST_GIT_BIN" add client1_file.txt
"$RUST_GIT_BIN" commit -m "Commit from client 1"
"$RUST_GIT_BIN" push origin
echo "✅ Client 1 pushed successfully."

# --- 5. Client 2 attempts a non-fast-forward push ---
echo "--- Client 2 attempting a non-fast-forward push (this must fail)... ---"
cd "$TEST_DIR/client2_repo"
# Client 2 is now out of date. It makes a different change on a divergent history.
echo "Change from client 2" >> client2_file.txt
"$RUST_GIT_BIN" add client2_file.txt
"$RUST_GIT_BIN" commit -m "Commit from client 2"

# This push MUST fail because the remote has commits that client 2 does not have.
echo "Verifying that non-fast-forward push is rejected..."
if "$RUST_GIT_BIN" push origin; then
    echo "❌ Test FAILED: Non-fast-forward push was accepted."
    exit 1
else
    echo "✅ Non-fast-forward push was correctly rejected."
fi

# --- 6. Client 2 pulls changes and then pushes ---
echo "--- Client 2 pulling to resolve conflict, then pushing... ---"
# We expect the pull to create a merge commit.
"$RUST_GIT_BIN" pull origin
echo "✅ Client 2 pulled and merged successfully."

# Now, client 2's history contains the server's changes, so a push should succeed.
"$RUST_GIT_BIN" push origin
echo "✅ Client 2's second push attempt successful."

# --- 7. Final Verification ---
echo "--- Verifying final server state... ---"
kill $SERVER_PID
SERVER_PID=""
sleep 1 # Give server time to shut down

cd "$TEST_DIR/server_repo"
"$RUST_GIT_BIN" checkout master

assert_exists "common_file.txt"
assert_exists "client1_file.txt"
assert_exists "client2_file.txt"
echo "✅ Server state is correct."

echo ""
echo "🎉 All collaboration tests passed successfully! 🎉"

exit 0 

function assert_exists() {
    if [ ! -e "$1" ]; then
        echo "❌ Assertion FAILED: $1 does not exist in $(pwd)."
        ls -la
        exit 1
    fi
} 