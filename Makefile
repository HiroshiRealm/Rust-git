# Makefile for rust-git project
# Provides packaging and testing functionality

# Project configuration
PROJECT_NAME := rust-git
BINARY_NAME := rust-git
ZIP_NAME := submit.zip
CARGO_FLAGS := --release
ONLINE_JUDGE_FLAGS := --release --features online_judge

# Directories
SRC_DIR := src
TEST_DIR := tests
TARGET_DIR := target
RELEASE_DIR := $(TARGET_DIR)/release
BINARY_PATH := $(RELEASE_DIR)/$(BINARY_NAME)

# Test scripts
TEST_SCRIPTS := sample1.sh sample2.sh sample3.sh sample4.sh

# Colors for output
GREEN := \033[32m
YELLOW := \033[33m
RED := \033[31m
RESET := \033[0m

.PHONY: help build pack test test-sample1 test-sample2 test-sample3 test-sample4 test-all clean clean-tests

# Default target
all: help

help:
	@echo "$(GREEN)rust-git Makefile$(RESET)"
	@echo "=================="
	@echo ""
	@echo "$(YELLOW)Available targets:$(RESET)"
	@echo "  $(GREEN)help$(RESET)         - Show this help message"
	@echo "  $(GREEN)build$(RESET)        - Build the project in release mode"
	@echo "  $(GREEN)pack$(RESET)         - Package the project for submission (like pack.sh)"
	@echo "  $(GREEN)test-all$(RESET)     - Run all test scripts"
	@echo "  $(GREEN)test-sample1$(RESET) - Run sample1.sh test (add and commit functionality)"
	@echo "  $(GREEN)test-sample2$(RESET) - Run sample2.sh test (branch and checkout functionality)"
	@echo "  $(GREEN)test-sample3$(RESET) - Run sample3.sh test"
	@echo "  $(GREEN)test-sample4$(RESET) - Run sample4.sh test"
	@echo "  $(GREEN)clean$(RESET)        - Clean build artifacts"
	@echo "  $(GREEN)clean-tests$(RESET)  - Clean test artifacts and temporary directories"
	@echo ""
	@echo "$(YELLOW)Examples:$(RESET)"
	@echo "  make build                    # Build the project"
	@echo "  make pack                     # Create submission package"
	@echo "  make test-sample1            # Run specific test"
	@echo "  make test-all                # Run all tests"
	@echo ""
	@echo "$(YELLOW)Test Details:$(RESET)"
	@echo "  Tests will:"
	@echo "  1. Build the project if needed"
	@echo "  2. Copy the binary to tests/ directory"
	@echo "  3. Execute the specified test script in tests/ directory"
	@echo "  4. Clean up temporary files and directories"
	@echo ""

# Build the project
build:
	@echo "$(GREEN)Building project in release mode...$(RESET)"
	cargo build $(CARGO_FLAGS)
	@if [ -f "$(BINARY_PATH)" ]; then \
		echo "$(GREEN)Build successful: $(BINARY_PATH)$(RESET)"; \
	else \
		echo "$(RED)Error: Binary not found at $(BINARY_PATH)$(RESET)"; \
		exit 1; \
	fi

# Package for submission (equivalent to pack.sh)
pack: 
	@echo "$(GREEN)Creating submission package...$(RESET)"
	@echo "Building with online judge features..."
	@if ! cargo build $(ONLINE_JUDGE_FLAGS); then \
		echo "$(RED)Error: Cargo build failed. Aborting packaging.$(RESET)"; \
		exit 1; \
	fi
	@echo "$(GREEN)Build successful.$(RESET)"
	@if [ ! -f "$(BINARY_PATH)" ]; then \
		echo "$(RED)Error: Release binary not found at $(BINARY_PATH). Aborting packaging.$(RESET)"; \
		exit 1; \
	fi
	@echo "Creating temporary packaging directory..."
	@PACKAGING_DIR=$$(mktemp -d); \
	if [ ! -d "$$PACKAGING_DIR" ]; then \
		echo "$(RED)Error: Failed to create temporary packaging directory. Aborting.$(RESET)"; \
		exit 1; \
	fi; \
	PROJECT_STAGING_DIR="$$PACKAGING_DIR/$(PROJECT_NAME)"; \
	mkdir -p "$$PROJECT_STAGING_DIR/target/release"; \
	echo "Copying files to staging directory..."; \
	cp -r $(SRC_DIR) "$$PROJECT_STAGING_DIR/"; \
	cp Cargo.toml "$$PROJECT_STAGING_DIR/"; \
	cp Cargo.lock "$$PROJECT_STAGING_DIR/"; \
	cp "$(BINARY_PATH)" "$$PROJECT_STAGING_DIR/target/release/"; \
	echo "Creating zip file $(ZIP_NAME)..."; \
	cd "$$PACKAGING_DIR"; \
	if ! zip -r "$(CURDIR)/$(ZIP_NAME)" "$(PROJECT_NAME)"; then \
		echo "$(RED)Error: Failed to create zip file. Aborting.$(RESET)"; \
		cd "$(CURDIR)"; \
		rm -rf "$$PACKAGING_DIR"; \
		exit 1; \
	fi; \
	cd "$(CURDIR)"; \
	rm -rf "$$PACKAGING_DIR"; \
	echo "$(GREEN)Packaging complete!$(RESET)"; \
	echo "$(ZIP_NAME) created successfully in the project root."

# Run all tests
test-all: $(addprefix test-,$(basename $(TEST_SCRIPTS)))
	@echo "$(GREEN)All tests completed!$(RESET)"

# Generic test runner function
define run_test
	@echo "$(GREEN)Running $1 test...$(RESET)"
	@$(MAKE) build
	@echo "Copying binary to tests directory..."
	@cp "$(BINARY_PATH)" "$(TEST_DIR)/"
	@echo "Executing test script: $1"
	@cd $(TEST_DIR) && \
	if [ -f "$1" ]; then \
		chmod +x "$1"; \
		if ./$1; then \
			echo "$(GREEN)✓ $1 passed!$(RESET)"; \
		else \
			echo "$(RED)✗ $1 failed!$(RESET)"; \
			cd ..; \
			$(MAKE) clean-tests; \
			exit 1; \
		fi; \
	else \
		echo "$(RED)Error: Test script $1 not found!$(RESET)"; \
		exit 1; \
	fi
	@$(MAKE) clean-tests
endef

# Individual test targets
test-sample1:
	$(call run_test,sample1.sh)

test-sample2:
	$(call run_test,sample2.sh)

test-sample3:
	$(call run_test,sample3.sh)

test-sample4:
	$(call run_test,sample4.sh)

# Generic test target for any sample
test-%:
	$(call run_test,$*.sh)

# Clean build artifacts
clean:
	@echo "$(GREEN)Cleaning build artifacts...$(RESET)"
	cargo clean
	@if [ -f "$(ZIP_NAME)" ]; then \
		rm "$(ZIP_NAME)"; \
		echo "Removed $(ZIP_NAME)"; \
	fi

# Clean test artifacts
clean-tests:
	@echo "$(GREEN)Cleaning test artifacts...$(RESET)"
	@if [ -f "$(TEST_DIR)/$(BINARY_NAME)" ]; then \
		rm "$(TEST_DIR)/$(BINARY_NAME)"; \
		echo "Removed $(TEST_DIR)/$(BINARY_NAME)"; \
	fi
	@cd $(TEST_DIR) && \
	for dir in test*; do \
		if [ -d "$$dir" ]; then \
			rm -rf "$$dir"; \
			echo "Removed test directory: $(TEST_DIR)/$$dir"; \
		fi; \
	done 