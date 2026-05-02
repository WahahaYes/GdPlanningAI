##@ Testing

.PHONY: test
test: test-rust test-godot ## Run all tests (Rust + Godot)

.PHONY: test-rust
test-rust: ## Run Rust tests only
	"$(MAKE)" -C addons/GdPlanningAI/rust test

.PHONY: test-godot
test-godot: ## Run Godot integration tests
	@echo "Running tests..."
	godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit

##@ Formatting

.PHONY: format
format: format-rust format-godot ## Format all source files (Rust + GDScript)

.PHONY: format-rust
format-rust: ## Format Rust source files with rustfmt
	"$(MAKE)" -C addons/GdPlanningAI/rust format

.PHONY: format-godot
format-godot: ## Format GDScript files with gdformat (requires gdtoolkit)
	git ls-files '*.gd' | xargs uv run gdformat

##@ Linting

.PHONY: lint-style
lint-style: ## Check GDScript and Rust files for style-guide violations
	uv run scripts/lint_style.py

##@ Documentation

.PHONY: check-docs
check-docs: ## Check consistency of README and LICENSE between root and addon directory
	@echo "Checking documentation consistency..."
	@diff README.md addons/GdPlanningAI/README.md || (echo "README.md differs between root and addon directory" && exit 1)
	@diff LICENSE.txt addons/GdPlanningAI/LICENSE.txt || (echo "LICENSE.txt differs between root and addon directory" && exit 1)
	@echo "Documentation is consistent"

.PHONY: sync-docs
sync-docs: ## Copy README and LICENSE from root to addon directory
	@echo "Syncing documentation to addon directory..."
	@cp README.md addons/GdPlanningAI/README.md
	@cp LICENSE.txt addons/GdPlanningAI/LICENSE.txt
	@echo "Documentation synced successfully"

##@ Help

.PHONY: help
help: ## Show this help message
	@awk 'BEGIN {FS = ":.*?## "}; /^##@ / {printf "\n\033[1m%s\033[0m\n", substr($$0, 5)}; /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)