# Research: Tests, Templates & Developer Tooling

**Date**: 2026-08-01 **Source**: `test/`, `addons/GdPlanningAI/rust/tests/`, `script_templates/`, Makefiles, project config — audited via explore agent (bg_8cdc2ba5) **Status**: Research dump — material for docs/CODEBASE_OVERVIEW.md

______________________________________________________________________

## GDScript tests (GUT framework, test/)

`.gutconfig.json`: `dirs ["res://test"]`, `include_subdirs true`, `log_level 2`, `should_exit true`. Run: `make test-godot` → `godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit` (grep-filtered; trailing `|| true` means the Make target never fails the build even on test failure).

### test/unit/ (2 files)

- **test_sim_object_proxy.gd** (4 tests): SimObjectProxy set/get/has property, type preservation, missing property, overwrite.
- **test_gdpai_blackboard.gd** (12 tests): property ops, get_dict/set_dict round-trip, GDPAI_OBJECTS → proxies, get_proxies_in_group / get_proxy_in_group, get_object_for, clone_for_simulation deep-copy isolation.

### test/integration/ (5 files)

- **test_blackboard_clone.gd** (3 tests): clone independence, type preservation, multi-clone isolation.
- **test_async_planner.gd** (11 tests): empty plan failure (goal_index -1); already-satisfied goal → success 0 cost; all-goals-satisfied picks highest reward; one-action plan; lower-cost preferred; failed precondition; cheaper deeper chain beats expensive direct; newer submission cancels older in-flight; precondition edge cases (missing property vs ==true/==false/ has_property all fail).
- **test_requirements_provisions.gd** (11 tests): Pickup→Eat chain satisfies hunger; Eat alone fails without provider; execution order Pickup-then-Eat; Eat succeeds when already holding food; provider-bound re-simulation; cheapest valid chain (3.0 vs 51.0); binding_in_set requires world group membership; wildcard fact matches specific + multiple requirements; GoTo wildcard → Pickup → Eat 3-action chain; two at_target requirements each get their own GoTo in order.
- **test_campfire_example_smoke.gd** (6 tests): real campfire_2d.tscn prefabs, ON_DEMAND agents: full cooking chain = 5 actions (2×GoTo, Dig, Cook, Eat) in order; fire too low → no over-depth plan; preemptive fire maintenance wins when fire reward > hunger; critical hunger + low fire → no plan; AddFuel not planned when full; Cook not planned when holding wood.
- **test_hunger_example_smoke.gd** (1 test): real hunger prefabs; full agent wanders; at hunger 30 plans GoTo→Shake Tree (2 actions); executing drops a real FoodObject; then plans GoTo→Pick Up Item→Eat Held Food (3 actions).

## Rust tests (addons/GdPlanningAI/rust/tests/, 11 files + common)

Run: `make test-rust` → `cargo test`. Integration tests construct PlannerEngine/SearchContext directly with mock callback-responder threads (no Godot runtime), using `gdplanningai_rust::logger::init_log_channel()` and a 5s watchdog.

- **planner_integration.rs** (6): single action satisfies goal; already- satisfied → empty plan; no valid plan → failure; unmet precondition fails; respects_max_depth; goal priority (goal_index selection).
- **best_cost_termination.rs** (2): BestCost returns cheapest; FirstComplete returns first-found.
- **budget_exhaustion.rs** (2): budget=0 → Pending(0) (not Complete); max_depth=1 blocks 2-action plan.
- **cancellation.rs** (1): slow custom validity check parks search; cancel_flag → fast failure.
- **requirement_provision_chaining.rs** (7): pickup→eat chaining; FactWildcard binds concrete ObjectRef; bindings injected during forward validation; custom precondition false prunes branch; BindingInSet respects world group; action cannot satisfy its own precondition; BindingInSet rejected when provider not in group.
- **plan_branch_insert_action.rs** (6): insert into empty branch; position shifting; greedy identical-requirement clearing; provider+consumer bindings; append-at-end shifting; precondition removal + new additions.
- **snapshot_evaluation.rs** (12): builtin precondition evaluation on snapshots (has_property, equal, greater/less, world target, cross-type int/float, custom → None, world objects, clone independence).
- **debug_tree_test.rs** (10): TreeDump builder — already-satisfied goal, dead-end root, completing candidate, cost pruning, excluded actions, fwd steps, multiple goal attempts, branch counting, format, nesting.
- **repro_infinite_loop.rs** (1): Verifying state converges (regression).
- **planner_simulation_helpers.rs** (12): tests against the planned process_simulation refactor API (collect_bindings_for_position, clear_initial_state_requirements, validate_action_against_current_state, evaluate_open_preconditions_for_position, clear_requirements_from_provisions, finalize_verified_branch).
- **common/mod.rs**: `create_test_agent`, `create_test_world`, `create_sim_object` helpers.

## Script templates (script_templates/)

- **Goal/template.gd** — compute_reward, get_desired_state, get_title, get_description stubs.
- **Action/template.gd** — most complete: title/description, validity checks, cost, preconditions, requirements, provisions, clone_for_plan, simulate_effect, pre/perform/post_perform.
- **GdPAIBehaviorConfig/template.gd** — @export props + \_populate.
- **PropertyUpdater/template.gd** — constructor params, initialize, update_properties decay/growth pattern.
- **GdPAIObjectData/template.gd** — get_group_labels, get_sim_properties, get_provided_actions stubs.

## Makefile targets

### Root Makefile

- Testing: `test`, `test-rust`, `test-godot` (grep-filtered, `|| true`), `test-godot-pipe-output` (full log to test_output.log).
- Formatting: `format`, `format-rust` (rustfmt), `format-godot` (fix_gd_spacing.py + gdformat via uv).
- Linting: `lint-style` → `uv run scripts/lint_style.py`.
- Docs: `check-docs` (diff README/LICENSE root vs addon), `sync-docs`.
- Editor: `launch-editor`, `addons-install`, `godot-pin`, `godot-pin-version VERSION=4.6-stable`, `godot-install-pinned`.
- Recording: `record-obs` (SCENE/DURATION/FPS/OUTPUT/FULLSCREEN), `record-obs-fullscreen`.

### addons/GdPlanningAI/rust/Makefile

- `build-release` (cargo build --release → copies .so/.dylib/.dll to addons/GdPlanningAI/bin/{linux,macos,windows}/), `build-debug`, `build-all`, `build-linux` (cross), `build-windows` (cross), `clean-binaries`.
- Dev: `test` (cargo test), `format` (rustfmt), `lint` (clippy; errors block), `lint-fix`, `clean`, `doc` (cargo doc).
- Cross: `setup-cross` (cargo install cross; Docker/Podman required).
- **Per AGENTS.md**: run `make build-release` after Rust changes before the GDScript suite picks them up.

## addons.jsonc (third-party addons)

- **gut** v9.6.0 (bitwes/Gut), subfolder addons/gut — installed via GodotEnv, gitignored, cached under .addons/. Editor plugin enabled.

## project.godot

- config_version=5; name "GdPlanningAI"; features ["4.7", "Forward Plus"]; **no main scene set** (run scenes directly).
- Autoload: `GdPAIAutoload="*uid://ol4q4tu0vtwo"`.
- Editor plugins: GdPlanningAI + gut.
- [dotnet] assembly_name "GdPlanningAI" (vestigial Mono section).
- .gdextension (`gdplanningai.gdextension`): entry_symbol `gdext_rust_init`, compatibility_minimum "4.5", libs → addons/GdPlanningAI/bin/{linux,macos, windows}/ debug+release.

## scripts/ (root)

- **capture_obs.py** — records a scene to video via OBS WebSocket (auto-start, .env credentials, source management).
- **obs_controller.py** — OBS WebSocket controller library.
- **capture_all_showcase.sh** — sequential showcase recording.
- **fix_gd_spacing.py** — enforces 2 blank lines before top-level funcs, 0 between ## docstrings and func.
- **lint_style.py** — style linter over git-tracked .gd/.rs (gd-walrus `:=` banned, gd-border, gd-spacing, gd-export docstring, gd-inline-lambda, rs-module doc, rs-pub-doc, rs-border).

## CI / Pre-commit

- **No `.github/` and no GitHub Actions** (verified).
- `.pre-commit-config.yaml` (pre-commit + pre-push, fail_fast false): pre-commit-hooks v5.0.0, cargo fmt/clippy (auto-fix + check) via rust Makefile, gitleaks v8.24.0 (.gitleaks.toml), mdformat 0.7.19 (--wrap=80), ruff v0.6.0 + ruff-format, gdformat + gdlint-style, check-docs.

## Rust build artifact flow

`make build-release` → cargo build --release → copies shared lib to addons/GdPlanningAI/bin/<platform>/; gdplanningai.gdextension loads it per platform (Linux .so, Windows .dll, macOS .dylib). Cross via `cross` + Cross.toml (Windows image ghcr.io/raniz85/cross/x86_64-pc-windows-gnu:24.04). Current artifacts: bin/linux/libgdplanningai_rust.so (~5.2MB real), bin/windows/gdplanningai_rust.dll (133 bytes placeholder). bin/.gitignore only ignores temp `**/~*` files — binaries committed.
