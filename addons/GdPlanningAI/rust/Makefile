# GdPlanningAI Rust Extension Makefile

# Directory paths
RUST_DIR = .
BIN_DIR = ../bin
TARGET_DIR = $(RUST_DIR)/target

# ======================================
# Build Commands
# ======================================

# Build the extension for current platform (release mode)
.PHONY: build-release
build-release:
	cargo build --release
	@echo "Copying release binary for current platform..."
	mkdir -p $(BIN_DIR)/linux 2>/dev/null || true
	mkdir -p $(BIN_DIR)/windows 2>/dev/null || true
	cp $(TARGET_DIR)/release/*.so $(BIN_DIR)/linux/ 2>/dev/null || true
	cp $(TARGET_DIR)/release/*.dll $(BIN_DIR)/windows/ 2>/dev/null || true

# Build the extension in debug mode
.PHONY: build-debug
build-debug:
	cargo build
	@echo "Copying debug binary for current platform..."
	mkdir -p $(BIN_DIR)/linux 2>/dev/null || true
	mkdir -p $(BIN_DIR)/windows 2>/dev/null || true
	cp $(TARGET_DIR)/debug/*.so $(BIN_DIR)/linux/ 2>/dev/null || true
	cp $(TARGET_DIR)/debug/*.dll $(BIN_DIR)/windows/ 2>/dev/null || true

# Build for all cross-platform targets (Linux + Windows)
.PHONY: build-all
build-all:
	@echo "Building for all cross-platform targets..."
	$(MAKE) build-linux
	$(MAKE) build-windows
	@echo "Cross-platform builds completed. Binaries copied to $(BIN_DIR)"

# Build for Linux (x86_64) using cross for cross-compilation
.PHONY: build-linux
build-linux:
	$(MAKE) clean
	@echo "Building for Linux x86_64..."
	cross build --release --target x86_64-unknown-linux-gnu
	mkdir -p $(BIN_DIR)/linux
	cp $(TARGET_DIR)/x86_64-unknown-linux-gnu/release/libgdplanningai_rust.so $(BIN_DIR)/linux/

# Build for Windows (x86_64) using cross with GNU target
.PHONY: build-windows
build-windows:
	$(MAKE) clean
	@echo "Building for Windows x86_64..."
	cross build --release --target x86_64-pc-windows-gnu
	mkdir -p $(BIN_DIR)/windows
	cp $(TARGET_DIR)/x86_64-pc-windows-gnu/release/gdplanningai_rust.dll $(BIN_DIR)/windows/


# Clean all binary directories
.PHONY: clean-binaries
clean-binaries:
	@echo "Cleaning all binary directories..."
	rm -rf $(BIN_DIR)/linux/*
	rm -rf $(BIN_DIR)/windows/*
	@echo "Binary directories cleaned"

# ======================================
# Development Commands
# ======================================

# Run unit tests (standalone, no Godot required)
.PHONY: test
test:
	cargo test

# Format code using rustfmt
.PHONY: format
format:
	rustup component add rustfmt && cargo fmt

# Run clippy linter
.PHONY: lint
lint:
	rustup component add clippy && cargo clippy

# Run clippy linter with autofix
.PHONY: lint-fix
lint-fix:
	rustup component add clippy && cargo clippy --fix

# Clean build artifacts
.PHONY: clean
clean:
	cargo clean

# Generate documentation
.PHONY: doc
doc:
	cargo doc --open

# ======================================
# Cross-Compilation Setup
# ======================================

# Install cross for cross-compilation (requires Docker or Podman)
.PHONY: setup-cross
setup-cross:
	@echo "Installing cross..."
	cargo install cross
	@echo "Cross installed. Ensure Docker or Podman is running."
	@echo "On Windows/macOS: Docker Desktop should be running."
	@echo "On Linux: Run 'sudo systemctl start docker' or use Podman."

# ======================================
# Help Command
# ======================================

.PHONY: help
help:
	@echo "GdPlanningAI Rust Extension Build System"
	@echo ""
	@echo "Build Commands:"
	@echo "  build-debug          Build debug binary for current platform"
	@echo "  build-release         Build release binary for current platform"
	@echo "  build-all            Build for all supported platforms"
	@echo "  build-linux          Build for Linux x86_64"
	@echo "  build-windows        Build for Windows x86_64"
	@echo "  clean-binaries       Clean all binary directories"
	@echo ""
	@echo "Development Commands:"
	@echo "  test                 Run unit tests"
	@echo "  test-all             Run all tests"
	@echo "  format               Format code"
	@echo "  lint                 Run linter"
	@echo "  lint-fix             Run linter with autofix"
	@echo "  clean                Clean build artifacts"
	@echo "  doc                  Generate documentation"
	@echo ""
	@echo "Setup Commands:"
	@echo "  setup-cross          Install cross (requires Docker/Podman)"
	@echo "  help                 Show this help message"
