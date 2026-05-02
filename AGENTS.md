# Agent Guidelines

Guidelines for AI agents working in this codebase.

## Documentation

Follow the documentation style guide: [docs/DOCUMENTATION_GUIDELINES.md](docs/DOCUMENTATION_GUIDELINES.md)

Plans scoped to the duration of one or few sessions should be written into `notes/` and continually updated.

## Build Tasks

Prefer `make` targets over executing arbitrary shell commands. Check the [Makefile](Makefile) for available targets before proposing custom commands.

New features should include unit tests and integration tests when applicable.  For `gdscript` code, these tests are written in the `test/` folder.  For `rust` planning engine updates, tests are bundled into `addons/GdPlanningAI/rust/tests/`.

If implementing `rust` code, an updated binary must be created with `make build-release` before running the `gdscript` test suite for changes to be properly picked up.

