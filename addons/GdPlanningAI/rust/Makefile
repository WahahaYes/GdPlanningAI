# Directory paths
RUST_DIR = .
BIN_DIR = ../bin
TARGET_DIR = $(RUST_DIR)/target

##@ Build

.PHONY: build-release
build-release: ## Build release binary for current platform
	@cargo build --release --quiet --message-format=short
	@echo "Copying release binary for current platform..."
	@if [ "$$(uname -s)" = "Linux" ]; then \
		mkdir -p $(BIN_DIR)/linux && \
		cp $(TARGET_DIR)/release/*.so $(BIN_DIR)/linux/; \
	elif [ "$$(uname -s)" = "Darwin" ]; then \
		mkdir -p $(BIN_DIR)/macos && \
		cp $(TARGET_DIR)/release/*.dylib $(BIN_DIR)/macos/; \
	else \
		mkdir -p $(BIN_DIR)/windows && \
		cp $(TARGET_DIR)/release/*.dll $(BIN_DIR)/windows/; \
	fi

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

.PHONY: build-all
build-all: ## Build for all supported platforms
	"$(MAKE)" build-linux
	"$(MAKE)" build-windows
	@echo "Binaries copied to $(BIN_DIR)"

.PHONY: build-linux
build-linux: ## Build for Linux x86_64
	@echo "Building for Linux x86_64..."
	@cargo build --release --quiet --message-format=short --target x86_64-unknown-linux-gnu
	@mkdir -p $(BIN_DIR)/linux
	@cp $(TARGET_DIR)/x86_64-unknown-linux-gnu/release/libgdplanningai_rust.so $(BIN_DIR)/linux/

.PHONY: build-windows
build-windows: ## Build for Windows x86_64 (needs setup-windows and mingw-w64)
	@echo "Building for Windows x86_64..."
	@cargo build --release --quiet --message-format=short --target x86_64-pc-windows-gnu
	@mkdir -p $(BIN_DIR)/windows
	@cp $(TARGET_DIR)/x86_64-pc-windows-gnu/release/gdplanningai_rust.dll $(BIN_DIR)/windows/

.PHONY: clean-binaries
clean-binaries: ## Clean all binary directories
	@echo "Cleaning all binary directories..."
	rm -rf $(BIN_DIR)/linux/*
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

.PHONY: lint
lint: ## Run clippy (errors block commit, warnings print only)
	@rustup component add clippy 2>&1 | grep -E "(error|Error)" || true
	@result=$$(cargo clippy --message-format=short 2>&1); status=$$?; \
	errors=$$(echo "$$result" | grep -cE ": error(\[|:)" || true); \
	warnings=$$(echo "$$result" | grep -cE ": warning(\[|:)" || true); \
	echo "$$result" | grep -vE "^\s*$$" || true; \
	echo "clippy: $$errors error(s), $$warnings warning(s)"; \
	exit $$status

.PHONY: lint-fix
lint-fix: ## Run clippy linter with autofix
	rustup component add clippy && cargo clippy --fix --allow-dirty --allow-no-vcs

.PHONY: clean
clean: ## Clean build artifacts
	@cargo clean 2>&1 | grep -E "(error|Error)" || true

.PHONY: doc
doc: ## Generate documentation
	cargo doc --open

##@ Cross-Compilation

.PHONY: setup-windows
setup-windows: ## Add the Windows rustup target (linker: apt install gcc-mingw-w64-x86-64)
	rustup target add x86_64-pc-windows-gnu

##@ Help

.PHONY: help
help: ## Show this help message
	@awk 'BEGIN {FS = ":.*?## "}; /^##@ / {printf "\n\033[1m%s\033[0m\n", substr($$0, 5)}; /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
