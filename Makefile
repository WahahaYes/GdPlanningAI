.PHONY: test
test: test-rust test-godot ## Run all tests (Rust + Godot)

.PHONY: test-rust
test-rust: ## Run Rust tests only
	cd addons/GdPlanningAI/rust && cargo test

.PHONY: test-godot
test-godot: ## Run Godot integration tests
	@echo "Running tests..."
	godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit

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

.PHONY: help
help: ## Show this help message
	@echo "GdPlanningAI Test Commands"
	@echo "=========================="
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'