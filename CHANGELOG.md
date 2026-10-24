# Changelog

## Unreleased

- **Built libraries are no longer in the repository.** The Linux and Windows libraries that used to sit under `addons/GdPlanningAI/bin` are gone, and a pre-commit hook and CI reject new ones. Releases carry working binaries in `GdPlanningAI-X.Y.Z.zip`; anyone using the GitHub source builds the library for their platform with `make -C addons/GdPlanningAI/rust build-release`.
- **Plan verification makes fewer main-thread calls.** Verifying a plan reuses the first step that discovery already simulated from the agent's current state, so a plan's first action is no longer simulated twice.

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

- **CI.** Every PR runs the pre-commit hooks, Rust format, clippy and tests, and the GUT suite on Linux, Windows and macOS against binaries built in the job. One `ci` check gates merging into main.
- **Releases are one command.** `scripts/release.sh X.Y.Z` bumps `plugin.cfg`, `Cargo.toml` and `Cargo.lock`, opens and merges the release PR, tags it, and the tag builds and publishes the release.
- **Quiet and verbose tests.** `make test`, `test-rust` and `test-godot` keep their short output; CI runs `test-rust-verbose` and `test-godot-verbose` for full logs.
- **Rust toolchain pinned** in `addons/GdPlanningAI/rust/rust-toolchain.toml`, and the Godot version in `global.json`.
