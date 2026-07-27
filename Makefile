##@ Testing

.PHONY: test
test: test-rust test-godot ## Run all tests (Rust + Godot)

.PHONY: test-rust
test-rust: ## Run Rust tests only
	"$(MAKE)" -C addons/GdPlanningAI/rust test

.PHONY: test-godot
test-godot: ## Run Godot integration tests
	@echo "Running tests..."
	@godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit 2>&1 | grep -E "(Failed|Error|PASSED|passed)" || true

.PHONY: test-godot-pipe-output
test-godot-pipe-output: ## Run Godot integration tests with full output to file
	@echo "Running tests with full output to test_output.log..."
	@rm -f test_output.log
	@godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit > test_output.log 2>&1

##@ Formatting

.PHONY: format
format: format-rust format-godot ## Format all source files (Rust + GDScript)

.PHONY: format-rust
format-rust: ## Format Rust source files with rustfmt
	"$(MAKE)" -C addons/GdPlanningAI/rust format

.PHONY: format-godot
format-godot: ## Format GDScript files with gdformat (requires gdtoolkit)
	@uv run scripts/fix_gd_spacing.py 2>&1 | grep -E "(error|Error)" || true
	@git ls-files '*.gd' | xargs uv run gdformat 2>&1 | grep -E "(error|Error)" || true

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

##@ Editor

.PHONY: launch-editor
launch-editor: ## Launch the Godot editor with this project (uses godotenv pinned version if available)
	@if command -v godotenv >/dev/null 2>&1; then \
		echo "Using godotenv pinned Godot version..."; \
		$$(godotenv godot env get) --editor --path .; \
	else \
		echo "Warning: godotenv not found, falling back to system godot"; \
		godot --editor --path .; \
	fi

##@ Addons

.PHONY: addons-install
addons-install: ## Install Godot addons from addons.jsonc
	godotenv addons install

##@ Godot Version

.PHONY: godot-pin
godot-pin: ## Pin current Godot version to .godotrc (uses godotenv)
	godotenv godot pin

.PHONY: godot-pin-version
godot-pin-version: ## Pin specific Godot version (usage: make godot-pin-version VERSION=4.6-stable)
	@if [ -z "$(VERSION)" ]; then echo "Usage: make godot-pin-version VERSION=4.6-stable"; exit 1; fi
	godotenv godot use $(VERSION) --no-dotnet
	godotenv godot pin

.PHONY: godot-install-pinned
godot-install-pinned: ## Install the pinned Godot version (uses godotenv)
	godotenv godot install --no-dotnet

##@ Help

.PHONY: help
help: ## Show this help message
	@awk 'BEGIN {FS = ":.*?## "}; /^##@ / {printf "\n\033[1m%s\033[0m\n", substr($$0, 5)}; /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

##@ Recording

# Defaults for recording
SCENE       ?= examples/hunger_basic_2d.tscn
DURATION    ?= 10
OUTPUT      ?= media/captures

.PHONY: record-obs
record-obs: ## Record scene via OBS (auto-loads .env)
	@bash -c 'set -a; test -f .env && . .env; set +a; \
		args="$(SCENE) -d $(DURATION) --start-obs"; \
		[ "$(FULLSCREEN)" = "1" ] && args="$$args -f"; \
		[ -n "$(OUTPUT)" ] && args="$$args -o $(OUTPUT)"; \
		uv run scripts/capture_obs.py $$args'

.PHONY: record-obs-fullscreen
record-obs-fullscreen: ## Record scene via OBS in fullscreen mode (-f)
	@$(MAKE) record-obs FULLSCREEN=1
