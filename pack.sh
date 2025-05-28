#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

PROJECT_NAME="rust-git"
ZIP_NAME="submit.zip"
SCRIPT_DIR=$(pwd) # Assuming the script is run from the project root

echo "Building project in release mode for Online Judge submission..."
# Build with the online_judge feature to ensure minimal output from the binary
if ! cargo build --release --features online_judge; then
    echo "Error: Cargo build failed. Aborting packaging."
    exit 1
fi

echo "Build successful."

# Path to the compiled binary
RELEASE_BINARY="target/release/${PROJECT_NAME}"

if [ ! -f "$RELEASE_BINARY" ]; then
    echo "Error: Release binary not found at ${RELEASE_BINARY}. Aborting packaging."
    exit 1
fi

echo "Creating temporary packaging directory..."
# Create a temporary directory for packaging
PACKAGING_DIR=$(mktemp -d)
if [ ! -d "$PACKAGING_DIR" ]; then
    echo "Error: Failed to create temporary packaging directory. Aborting."
    exit 1
fi

# Create the required directory structure inside the packaging directory
PROJECT_STAGING_DIR="${PACKAGING_DIR}/${PROJECT_NAME}"
mkdir -p "${PROJECT_STAGING_DIR}/target/release"

echo "Copying files to staging directory..."
# Copy source files
cp -r src "${PROJECT_STAGING_DIR}/"
# Copy Cargo files
cp Cargo.toml "${PROJECT_STAGING_DIR}/"
cp Cargo.lock "${PROJECT_STAGING_DIR}/"
# Copy the release binary
cp "${RELEASE_BINARY}" "${PROJECT_STAGING_DIR}/target/release/"

echo "Creating zip file ${ZIP_NAME}..."
# Go into the packaging directory to create the zip with the correct internal structure
cd "${PACKAGING_DIR}"
if ! zip -r "${SCRIPT_DIR}/${ZIP_NAME}" "${PROJECT_NAME}"; then
    echo "Error: Failed to create zip file. Aborting."
    # Clean up temp dir even on zip failure before exiting
    cd "${SCRIPT_DIR}" # Go back to original directory
    echo "Cleaning up temporary packaging directory: ${PACKAGING_DIR}"
    rm -rf "${PACKAGING_DIR}"
    exit 1
fi

# Go back to the original directory
cd "${SCRIPT_DIR}"

echo "Cleaning up temporary packaging directory: ${PACKAGING_DIR}"
rm -rf "${PACKAGING_DIR}"

echo ""
echo "Packaging complete!"
echo "${ZIP_NAME} created successfully in the project root."
echo "It contains:"
echo "- /${PROJECT_NAME}"
echo "  - /src"
echo "    - ..."
echo "  - ./target/release/${PROJECT_NAME}"
echo "  - Cargo.toml"
echo "  - Cargo.lock"
echo ""
echo "To use it:"
echo "1. Make sure it's executable: chmod +x package_for_oj.sh"
echo "2. Run it from your project root: ./package_for_oj.sh" 