.PHONY: test
test: test-rust test-godot ## Run all tests (Rust + Godot)

.PHONY: test-rust
test-rust: ## Run Rust tests only
	cd addons/GdPlanningAI/rust && cargo test

.PHONY: test-godot
test-godot: ## Run Godot integration tests
	@echo "Running tests..."
	godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit

.PHONY: help
help: ## Show this help message
	@echo "GdPlanningAI Test Commands"
	@echo "=========================="
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'