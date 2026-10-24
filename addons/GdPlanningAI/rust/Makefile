SHELL := /bin/bash

# Directory paths
RUST_DIR = .
BIN_DIR = ../bin
ROOT_DIR = ../../..
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

# Each OS builds its own binary natively (CI does all three; the release workflow packages them).
# macOS ships one universal dylib: aarch64 and x86_64 slices joined with lipo.
MAC_TARGETS := aarch64-apple-darwin x86_64-apple-darwin
MAC_DYLIB := libgdplanningai_rust.dylib

# Coverage. Its builds live in their own target dir, so dropping the remap flags rebuilds nothing
# else; the report needs real paths to find the sources. Test files are not counted.
CARGO_LLVM_COV_VERSION := 0.9.1
COVERAGE_DIR := $(TARGET_DIR)/llvm-cov
COVERAGE_IGNORE := --ignore-filename-regex '/tests/'
# An instrumented Godot run writes default_*.profraw into its working directory, the project root.
COVERAGE_TOOL_CHECK = cargo llvm-cov --version 2>/dev/null | grep -qx "cargo-llvm-cov $(CARGO_LLVM_COV_VERSION)" || \
	{ echo "cargo-llvm-cov $(CARGO_LLVM_COV_VERSION) not installed: run make setup"; exit 1; }
coverage build-instrumented coverage-behavioral-report: RUSTFLAGS :=

##@ Setup

.PHONY: setup
setup: ## Install the pinned toolchain (rustfmt, clippy) and the coverage tools; safe to rerun
	@rustup toolchain install
	@rustup component add llvm-tools-preview
	@cargo llvm-cov --version 2>/dev/null | grep -qx "cargo-llvm-cov $(CARGO_LLVM_COV_VERSION)" || \
		cargo install cargo-llvm-cov --version $(CARGO_LLVM_COV_VERSION) --locked

##@ Build

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

.PHONY: test-verbose
test-verbose: ## Run unit tests with cargo's full output; propagates the exit code (CI)
	cargo test

.PHONY: coverage
coverage: ## Run the Rust tests instrumented (CI's Rust test step): summary, HTML and LCOV in target/llvm-cov
	@$(COVERAGE_TOOL_CHECK)
	@cargo llvm-cov clean --workspace
	@cargo llvm-cov --no-report
	@cargo llvm-cov report --html --output-dir $(COVERAGE_DIR) $(COVERAGE_IGNORE)
	@cargo llvm-cov report --lcov --output-path $(COVERAGE_DIR)/lcov.info $(COVERAGE_IGNORE)
	@cargo llvm-cov report --summary-only $(COVERAGE_IGNORE)
	@echo "HTML report: $(COVERAGE_DIR)/html/index.html"

# The behavioral flag: the GUT and e2e suites against an instrumented library (Linux). CI runs
# build-instrumented, the suites, then coverage-behavioral-report; coverage-behavioral does all of
# it locally. cargo-llvm-cov's environment makes the plain release build instrumented, so it
# replaces target/release and the library in ../bin until the next build-release.
.PHONY: build-instrumented
build-instrumented: ## Build the instrumented library into ../bin/linux (for coverage-behavioral-report)
	@$(COVERAGE_TOOL_CHECK)
	@[ "$$(uname -s)" = Linux ] || { echo "behavioral coverage runs on Linux only"; exit 1; }
	@rm -f $(TARGET_DIR)/*.profraw $(TARGET_DIR)/*.profdata $(TARGET_DIR)/rust-profraw-list \
		$(ROOT_DIR)/default_*.profraw
	@eval "$$(cargo llvm-cov show-env --sh)" && \
		cargo build --release --lib --quiet --message-format=short && \
		mkdir -p $(BIN_DIR)/linux && cp $(TARGET_DIR)/release/*.so $(BIN_DIR)/linux/
	@echo "Instrumented library in $(BIN_DIR)/linux: make build-release puts the normal one back"

.PHONY: coverage-behavioral-report
coverage-behavioral-report: ## Report what the Godot runs since build-instrumented covered: LCOV in target/llvm-cov/lcov-behavioral.info
	@$(COVERAGE_TOOL_CHECK)
	@ls $(ROOT_DIR)/default_*.profraw > /dev/null 2>&1 || { echo "no Godot profiles: run the suites after build-instrumented"; exit 1; }
	@mv $(ROOT_DIR)/default_*.profraw $(TARGET_DIR)/
	@mkdir -p $(COVERAGE_DIR)
	@eval "$$(cargo llvm-cov show-env --sh)" && \
		cargo llvm-cov report --release --lcov --output-path $(COVERAGE_DIR)/lcov-behavioral.info $(COVERAGE_IGNORE) && \
		cargo llvm-cov report --release --summary-only $(COVERAGE_IGNORE)

.PHONY: coverage-behavioral
coverage-behavioral: build-instrumented ## Behavioral coverage in one go: instrumented build, GUT, e2e, report
	@cd $(ROOT_DIR) && godot --headless --import > /dev/null && $(MAKE) test-godot && $(MAKE) test-e2e
	@$(MAKE) coverage-behavioral-report

.PHONY: format
format: ## Format code using rustfmt
	@cargo fmt

.PHONY: format-check
format-check: ## Check formatting with rustfmt (no rewrite)
	@cargo fmt --check

.PHONY: lint
lint: ## Run clippy; warnings fail
	@cargo clippy --all-targets --message-format=short -- -D warnings

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
