.PHONY: test test-rust test-godot help

help:
	@echo "GdPlanningAI Test Commands"
	@echo "=========================="
	@echo "make test        - Run all tests (Rust + Godot)"
	@echo "make test-rust   - Run Rust tests only"
	@echo "make test-godot  - Run Godot integration tests"

test: test-rust test-godot

test-rust:
	cd addons/GdPlanningAI/rust && cargo test

test-godot:
	@echo "Importing Godot resources..."
	@godot --headless --import --quit
	@echo "Running tests..."
	godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit

test-godot-verbose:
	@echo "Importing Godot resources..."
	@godot --headless --import --quit
	@echo "Running tests..."
	godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit -gpo
