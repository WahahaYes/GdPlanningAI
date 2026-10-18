# Directory paths
RUST_DIR = .
BIN_DIR = ../bin
TARGET_DIR = $(RUST_DIR)/target

# Keep local paths out of the committed binaries, so builds match across checkouts. Every
# target shares these flags; changing RUSTFLAGS between targets would rebuild everything.
# filter-out keeps recursive makes from adding them twice. RUSTFLAGS overrides any
# rustflags in a cargo config.
CARGO_HOME ?= $(HOME)/.cargo
REMAP_FLAGS := --remap-path-prefix=$(CURDIR)=/gdplanningai_rust \
	--remap-path-prefix=$(CARGO_HOME)=/cargo \
	--remap-path-prefix=$(shell rustc --print sysroot)=/rustc
export RUSTFLAGS := $(strip $(filter-out $(REMAP_FLAGS),$(RUSTFLAGS)) $(REMAP_FLAGS))

##@ Build

# Each OS builds its own binary natively (CI does all three; see scripts/binaries.sh).
# macOS ships one universal dylib: aarch64 and x86_64 slices joined with lipo.
MAC_TARGETS := aarch64-apple-darwin x86_64-apple-darwin
MAC_DYLIB := libgdplanningai_rust.dylib

.PHONY: build-release
build-release: ## Build the release binary for this OS into ../bin (macOS: universal)
	@if [ "$$(uname -s)" = "Linux" ]; then \
		cargo build --release --quiet --message-format=short && \
		mkdir -p $(BIN_DIR)/linux && \
		cp $(TARGET_DIR)/release/*.so $(BIN_DIR)/linux/; \
	elif [ "$$(uname -s)" = "Darwin" ]; then \
		for t in $(MAC_TARGETS); do \
			cargo build --release --quiet --message-format=short --target $$t || exit 1; \
		done && \
		mkdir -p $(BIN_DIR)/macos && \
		lipo -create -output $(BIN_DIR)/macos/$(MAC_DYLIB) \
			$(foreach t,$(MAC_TARGETS),$(TARGET_DIR)/$(t)/release/$(MAC_DYLIB)) && \
		codesign --force --sign - $(BIN_DIR)/macos/$(MAC_DYLIB); \
	else \
		cargo build --release --quiet --message-format=short && \
		mkdir -p $(BIN_DIR)/windows && \
		cp $(TARGET_DIR)/release/*.dll $(BIN_DIR)/windows/; \
	fi
	@echo "Release binary copied to $(BIN_DIR)"

.PHONY: build-debug
build-debug: ## Build debug binary for current platform
	@cargo build --quiet --message-format=short
	@echo "Copying debug binary for current platform..."
	@if [ "$$(uname -s)" = "Linux" ]; then \
		mkdir -p $(BIN_DIR)/linux && \
		cp $(TARGET_DIR)/debug/*.so $(BIN_DIR)/linux/; \
	elif [ "$$(uname -s)" = "Darwin" ]; then \
		mkdir -p $(BIN_DIR)/macos && \
		cp $(TARGET_DIR)/debug/*.dylib $(BIN_DIR)/macos/; \
	else \
		mkdir -p $(BIN_DIR)/windows && \
		cp $(TARGET_DIR)/debug/*.dll $(BIN_DIR)/windows/; \
	fi

.PHONY: clean-binaries
clean-binaries: ## Clean all binary directories
	@echo "Cleaning all binary directories..."
	rm -rf $(BIN_DIR)/linux/*
	rm -rf $(BIN_DIR)/macos/*
	rm -rf $(BIN_DIR)/windows/*
	@echo "Binary directories cleaned"

##@ Development

SHELL := /bin/bash

.PHONY: test
test: ## Run unit tests (full log capture; CARGO-TEST-OK/FAILED verdict; propagates cargo exit code)
	@echo "Running tests..."
	@LOG=$$(mktemp /tmp/cargo-test.XXXXXX); \
	cargo test --message-format=short > $$LOG 2>&1; \
	STATUS=$$?; \
	cat $$LOG; \
	if [ $$STATUS -ne 0 ]; then \
		echo "CARGO-TEST-FAILED: cargo test exited with status $$STATUS (full log at $$LOG)"; \
	else \
		echo "CARGO-TEST-OK"; \
	fi; \
	exit $$STATUS

.PHONY: format
format: ## Format code using rustfmt
	@rustup component add rustfmt 2>&1 | grep -E "(error|Error)" || true
	@cargo fmt

.PHONY: format-check
format-check: ## Check formatting with rustfmt (no rewrite)
	@cargo fmt --check

.PHONY: lint
lint: ## Run clippy; warnings fail
	@rustup component add clippy 2>&1 | grep -E "(error|Error)" || true
	@cargo clippy --message-format=short -- -D warnings

.PHONY: clean
clean: ## Clean build artifacts
	@cargo clean 2>&1 | grep -E "(error|Error)" || true

.PHONY: doc
doc: ## Generate documentation
	cargo doc --open

##@ Help

.PHONY: help
help: ## Show this help message
	@awk 'BEGIN {FS = ":.*?## "}; /^##@ / {printf "\n\033[1m%s\033[0m\n", substr($$0, 5)}; /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
