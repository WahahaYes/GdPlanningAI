# Changelog

## Unreleased

### For contributors

- **Godot 4.2 floor documented.** The addon's floor stays 4.2 (the `api-4-2` crate feature and `compatibility_minimum`, smoke-tested by `make test-compat`); the repository's own project, tests and examples use the pinned Godot 4.7, and the docs now say so.
- **Style checks come from necronomicon's packs.** necro 0.5 runs its `rust`, `rust-style`, `gdscript` and `gdscript-style` packs (gdformat, gdlint, `.gd.uid` tracking, the style guide) beside this repo's own rules; `scripts/lint_style.py` and the gdtoolkit wrapper are gone, and the Makefile's GDScript, style and catalog targets fold into `make necro` (check) and `make necro-fix` (spacing fixer, then `necro --fix`). The hook runs `necro --fix` on the changed files, so gdformat fixes are applied and the commit fails until they're re-staged.

## 0.4.0 - 2026-11-06

- **Plan delivery for turn-based games.** `GdPAIAgent` emits `plan_delivered(result)` whenever a plan result is installed, and adds `get_current_action()` and `is_waiting_for_plan()`.
- **Godot 4.2 and newer.** The library is built with godot-rust 0.5 against the Godot 4.2 API, so the addon loads on 4.2 and later (it claimed 4.5); CI tests it on 4.2.
- **Built libraries are no longer in the repository.** Install from the release zip `GdPlanningAI-X.Y.Z.zip`, or build from source with `make -C addons/GdPlanningAI/rust build-release`.
- **Faster planning.** Actions that declare `get_effect_spec` apply their effects in Rust, snapshots are shared instead of copied, and a plan's first step is no longer simulated twice. `GdPAIAgentConfig.max_effect_fallbacks` (default 128) caps GDScript effect calls per search.
- **Clearer logs.** Malformed spec entries now warn instead of being dropped silently, and the freed-object callback notice moved to debug.
- **Objects on a blackboard are read-only in simulation.** Changing one in `simulate_effect` changes the real object for every NPC; see [docs/AUTHORING_GUIDE.md](docs/AUTHORING_GUIDE.md).
- **Example scenes.** The 3D examples get a visual pass (heavier on the GPU), and one switch hides every debug label: `show_debug_text`, the H key or `DebugText.set_shown()`. It replaces `ManyAgentsSpawner3D.hide_debug_labels`.

## 0.3.0 - 2026-10-18

- **The planner is written in Rust.** Searches run on background threads, off the main thread; agents, goals and actions are still written in GDScript. The addon now needs its native library, so a release ships `GdPlanningAI-X.Y.Z.zip` with Linux, Windows and macOS (universal) binaries in `addons/GdPlanningAI/bin`. GitHub's own source archives hold Git LFS pointers instead of binaries, so install from the zip. [docs/ALGORITHM.md](docs/ALGORITHM.md) explains how a plan is found.
- **macOS support.** The universal macOS library is new; before this release no macOS binary was shipped.
- **Library version check.** The Rust library reports its version (`GdPAIPlanScheduler.get_engine_version`), and the addon warns at startup when it differs from `plugin.cfg`, which means a stale or mismatched binary.
- **Planning is cheaper and steadier.** Verified steps are reused across branches that share a chain prefix, branch snapshots are shared instead of cloned, each planning cycle has a time slice (`time_slice_ms`), and finished engines are freed off the main thread. A failed plan backs off before the agent plans again (`failure_backoff`), and the backoff clears on success.
- **Plans are checked more carefully.** Verification re-checks custom action preconditions and the goal after the last action, an action that only serves the goal is placed at the end of the chain, plans with open requirements are never chosen as the best plan, and goals are processed by priority. Provisions and bindings chain through the plan, so plan costs use the binding that execution uses, and the example wander action plans as a step instead of a simulated position.
- **Debugging.** `GdPAIPlanScheduler.get_debug_tree(agent)` prints the agent's last planning search, including which precondition failed verification. Log verbosity is one level in `plugin.cfg` (`log_level`, 0 to 3), and each plan logs a one-line `RESULT` summary. The interactive debugger tab from 0.2.0 is gone and has not been brought back.
- **Breaking: custom preconditions are created inline.** Build one with `Precondition.custom(func(blackboard, world_state): ...)` directly in an `append()` call, instead of making an object and setting `eval_func` first. Existing code that sets up preconditions the old way needs updating.
- **New 3D examples and a rewritten tutorial.** `campfire_3d`, `campfire_large_3d` (agents tend fires in pairs, with navmesh carving around campfires and wood piles) and `agent_showcase_3d`, alongside the 2D scenes. The docs are rewritten: the authoring guide with the timing contract of every GDScript hook, the examples as a tutorial with a reading order, and the planning algorithm.
- **Release archive.** Each release attaches `GdPlanningAI-X.Y.Z.zip`, the addon with its binaries for all three platforms.

### For contributors

- **CI.** Every PR runs the hooks in `.pre-commit-config.yaml`, Rust format, clippy and tests, and the GUT suite on Linux, Windows and macOS against binaries built in the job. One `ci` check gates merging into main.
- **Releases are one command.** `scripts/release.sh X.Y.Z` bumps `plugin.cfg`, `Cargo.toml` and `Cargo.lock`, opens and merges the release PR, tags it, and the tag builds and publishes the release.
- **Quiet and verbose tests.** `make test`, `test-rust` and `test-godot` keep their short output; CI runs `test-rust-verbose` and `test-godot-verbose` for full logs.
- **Rust toolchain pinned** in `addons/GdPlanningAI/rust/rust-toolchain.toml`, and the Godot version in `global.json`.
