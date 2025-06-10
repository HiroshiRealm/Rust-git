#!/bin/bash

# This test verifies the garbage collection ('gc') command, specifically
# its ability to pack loose objects and perform delta compression.

set -e

echo "--- Rust-Git GC Delta Compression Test ---"

# --- Configuration ---
PROJECT_ROOT=$(cd -- "$(dirname -- "$0")/../.." &> /dev/null && pwd)
RUST_GIT_BIN="$PROJECT_ROOT/target/release/rust-git"
TEST_DIR="/tmp/rust-git-gc-test"

# --- Helper Functions ---
function assert_exists() {
    if [ ! -e "$1" ]; then
        echo "❌ Assertion FAILED: $1 does not exist."
        exit 1
    fi
}

function get_loose_objects_size() {
    # It might return empty if no files are found, so default to 0.
    find .git/objects -type f -not -path "*.pack" -not -path "*.idx" -exec du -cb {} + | grep total | awk '{print $1}' || echo 0
}

# --- Cleanup function ---
cleanup() {
  echo "--- Cleaning up ---"
  echo "Removing test directory: $TEST_DIR"
  rm -rf "$TEST_DIR"
}

trap cleanup EXIT

# --- 1. Build & Setup ---
echo "--- Building project (release mode) ---"
cargo build --release

echo "--- Setting up test environment in $TEST_DIR ---"
REPO_DIR="$TEST_DIR/test_repo"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR" || exit

# --- Test Steps ---
echo "--- Initializing repository and creating objects using rust-git ---"
"$RUST_GIT_BIN" init

# Create a larger base file
head -c 10000 /dev/urandom | base64 > big_file.txt
"$RUST_GIT_BIN" add big_file.txt
"$RUST_GIT_BIN" commit -m "Add large base file"

# Create a small modification to the same file
cp big_file.txt big_file_modified.txt
echo "A small modification at the end." >> big_file_modified.txt
mv big_file_modified.txt big_file.txt
"$RUST_GIT_BIN" add big_file.txt
"$RUST_GIT_BIN" commit -m "Add small modification to large file"


echo "--- Verifying loose objects have been created ---"
BEFORE_SIZE=$(get_loose_objects_size)
if [ "$BEFORE_SIZE" -gt 20000 ]; then
    echo "✅ Loose objects created."
else
    echo "❌ Failed to create enough loose objects. Size: $BEFORE_SIZE"
    exit 1
fi
echo "Total size of loose objects before GC: $BEFORE_SIZE bytes"

echo "--- Running 'gc' ---"
"$RUST_GIT_BIN" gc

# --- Verification ---
echo "--- Verifying GC results ---"
PACK_DIR=".git/objects/pack"
assert_exists "$PACK_DIR"
PACK_FILE=$(find "$PACK_DIR" -name "*.pack")
IDX_FILE=$(find "$PACK_DIR" -name "*.idx")
assert_exists "$PACK_FILE"
assert_exists "$IDX_FILE"

AFTER_SIZE=$(stat -c%s "$PACK_FILE")
if [ "$AFTER_SIZE" -gt 0 ]; then
    echo "✅ Pack and index files were created."
else
    echo "❌ Pack file is empty."
    exit 1
fi

NUM_LOOSE_OBJECTS=$(find .git/objects -type f -not -path "*.pack" -not -path "*.idx" | wc -l)
if [ "$NUM_LOOSE_OBJECTS" -eq 0 ]; then
    echo "✅ Loose objects were cleaned up."
else
    echo "❌ Loose objects were not cleaned up. Found $NUM_LOOSE_OBJECTS remaining loose objects."
    find .git/objects -type f -not -path "*.pack" -not -path "*.idx"
    exit 1
fi

echo "Size of pack file after GC: $AFTER_SIZE bytes"
if [ "$AFTER_SIZE" -lt "$BEFORE_SIZE" ]; then
    echo "✅ Delta compression was effective (Pack file size: $AFTER_SIZE < Original size: $BEFORE_SIZE)."
else
    echo "❌ Delta compression was not effective."
    exit 1
fi

# A simple binary grep for the OFS_DELTA object type (6).
if hexdump -C "$PACK_FILE" | grep -q ' c'; then
    echo "✅ Found a delta object entry in the pack file."
else
    echo "❌ Could not confirm a delta object was created in the pack file."
    exit 1
fi

echo ""
echo "🎉 All GC and delta compression tests passed successfully! 🎉"

exit 0 