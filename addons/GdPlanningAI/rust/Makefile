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
	@echo "Building for all cross-platform targets..."
	"$(MAKE)" build-linux
	"$(MAKE)" build-windows
	@echo "Cross-platform builds completed. Binaries copied to $(BIN_DIR)"

.PHONY: build-linux
build-linux: ## Build for Linux x86_64
	"$(MAKE)" clean
	@echo "Building for Linux x86_64..."
	@cross build --release --target x86_64-unknown-linux-gnu --message-format=short 2>&1 | grep -E "(error|Error|warning|Warning)" || true
	@mkdir -p $(BIN_DIR)/linux
	@cp $(TARGET_DIR)/x86_64-unknown-linux-gnu/release/libgdplanningai_rust.so $(BIN_DIR)/linux/

.PHONY: build-windows
build-windows: ## Build for Windows x86_64
	"$(MAKE)" clean
	@echo "Building for Windows x86_64..."
	@cross build --release --target x86_64-pc-windows-gnu --message-format=short 2>&1 | grep -E "(error|Error|warning|Warning)" || true
	@mkdir -p $(BIN_DIR)/windows
	@cp $(TARGET_DIR)/x86_64-pc-windows-gnu/release/gdplanningai_rust.dll $(BIN_DIR)/windows/

.PHONY: clean-binaries
clean-binaries: ## Clean all binary directories
	@echo "Cleaning all binary directories..."
	rm -rf $(BIN_DIR)/linux/*
	rm -rf $(BIN_DIR)/windows/*
	@echo "Binary directories cleaned"

##@ Development

.PHONY: test
test: ## Run unit tests
	@cargo test --message-format=short 2>&1 | grep -E "(test result|error|Error|FAILED|failed)" || true

.PHONY: format
format: ## Format code using rustfmt
	@rustup component add rustfmt 2>&1 | grep -E "(error|Error)" || true
	@cargo fmt 2>&1 | grep -E "(error|Error)" || true

.PHONY: lint
lint: ## Run clippy linter
	@rustup component add clippy 2>&1 | grep -E "(error|Error)" || true
	@cargo clippy --message-format=short 2>&1 | grep -E "(error|Error|warning|Warning)" || true

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

.PHONY: setup-cross
setup-cross: ## Install cross (requires Docker/Podman)
	@echo "Installing cross..."
	cargo install cross
	@echo "Cross installed. Ensure Docker or Podman is running."
	@echo "On Windows/macOS: Docker Desktop should be running."
	@echo "On Linux: Run 'sudo systemctl start docker' or use Podman."

##@ Help

.PHONY: help
help: ## Show this help message
	@awk 'BEGIN {FS = ":.*?## "}; /^##@ / {printf "\n\033[1m%s\033[0m\n", substr($$0, 5)}; /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
