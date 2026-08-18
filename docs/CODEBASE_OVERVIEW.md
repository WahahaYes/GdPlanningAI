# Codebase Overview

A developer-oriented map of the GdPlanningAI repository. Read this first to figure out which file, module, or test you need to touch for any task. It covers the repository layout, the addon anatomy, the two-language architecture, the class hierarchy, the runtime flow, the build system, testing, developer tooling, and the documentation map.

For the search algorithm itself, see [docs/ALGORITHM.md](ALGORITHM.md) and [docs/PLANNER_PSEUDOCODE.md](PLANNER_PSEUDOCODE.md). For authoring actions and goals, see [docs/AUTHORING_GUIDE.md](AUTHORING_GUIDE.md). For step-by-step walkthroughs of the demo scenes, see [docs/EXAMPLES.md](EXAMPLES.md).

______________________________________________________________________

## 1. Repository layout

The top-level of the repo:

| Path | Purpose | |------|---------| | `addons/` | The plugin itself: `addons/GdPlanningAI/` is the installable addon. | | `docs/` | Project documentation (see [section 9](#9-documentation-map)). | | `examples/` | Demonstration scenes (hunger, campfire, multi-agent, stress test). | | `notes/` | Session-scoped research and plans; scratch material, not reference docs. | | `script_templates/` | Godot script templates for subclassing `Action`, `Goal`, `GdPAIObjectData`, and friends. | | `scripts/` | Developer tooling scripts (OBS recording, style linting). | | `test/` | GUT GDScript test suite (`test/unit/`, `test/integration/`). | | `media/` | Demo assets used by the README (banner, gifs, screenshots). | | `.worktrees/` | Git worktree checkouts used during development (numbered milestones like `01_before_pure_gdscript`). | | `.omo/` | omo tooling state (for example, run-continuation hooks). |

Other files of note at the root: `Makefile` (see [section 6](#6-build-system)), `project.godot` (Godot 4.7 project, no main scene), `addons.jsonc` (third-party addon manifest, currently `gut` v9.6.0), `.pre-commit-config.yaml`, and `AGENTS.md` (agent working guidelines).

______________________________________________________________________

## 2. Addon anatomy

`addons/GdPlanningAI/` is the Godot addon that gets copied into a user's project. It is also a full Godot project in its own right when opened at the repo root. The key files:

```text
addons/GdPlanningAI/
├── plugin.gd                 # EditorPlugin; registers the autoload singleton
├── plugin.cfg                # Plugin metadata (name, version, log_level)
├── gdpai_autoload.gd         # Autoload singleton GdPAIAutoload
├── gdpai_utils.gd            # GdPAIUTILS helpers (get_child_of_type, etc.)
├── gdplanningai.gdextension  # GDExtension config; points at bin/<platform> libs
├── bin/                      # Built Rust shared libraries (linux/macos/windows)
├── rust/                     # Rust planning engine crate (see section 3)
└── scripts/
    ├── gdpai_rust_bridge.gd      # GdPAIRustBridge serialization layer
    ├── gdpai_blackboard_plan.gd  # GdPAIBlackboardPlan resource
    ├── nodes/                    # GdPAIAgent, GdPAIWorldNode, GdPAIObjectData,
    │                             #   GdPAILocationData, GdPAIInteractable
    ├── refcounteds/              # Action, Goal, Precondition, specs, PropertyUpdater
    └── resources/                # GdPAIAgentConfig, GdPAIBehaviorConfig
```

- `plugin.gd` is `@tool extends EditorPlugin`. Its only job is to register the `GdPAIAutoload` autoload in `_enter_tree`.
- `gdpai_autoload.gd` creates the `GdPAIPlanScheduler` in `_ready` and calls `_scheduler.process_callbacks()` every frame in `_process`. That drain is what shuttles results and callback requests between the background planner threads and the main thread. It also applies the log level from `plugin.cfg`.
- `gdplanningai.gdextension` declares the entry symbol `gdext_rust_init`, `compatibility_minimum "4.5"`, and loads the platform shared library from `addons/GdPlanningAI/bin/{linux,macos,windows}/`.
- The Rust-exposed classes (`GdPAIBlackboard`, `SimObjectProxy`, `GdPAIPlanScheduler`) are registered via the `.gdextension`; they have no `.gd` file.

______________________________________________________________________

## 3. Two-language architecture

The addon is split across two languages with a strict division of labor.

### 3.1 GDScript API layer (what users write)

Everything under `scripts/` is the user-facing API. Users subclass `Action`, `Goal`, `GdPAIObjectData`, `Precondition`, `RequirementSpec`, `ProvisionSpec`, `PropertyUpdater`, `GdPAIBehaviorConfig`, and `GdPAIAgentConfig` to describe what their agents can do. `GdPAIAgent` and `GdPAIWorldNode` are the runtime nodes. `GdPAIRustBridge` is the internal serialization layer between the two languages. See [section 4](#4-class-hierarchy-overview) for the full class map.

### 3.2 Rust planner engine (the brains)

The Rust crate lives in `addons/GdPlanningAI/rust/`. Crate `gdplanningai-rust` v0.1.0, edition 2024, `crate-type = ["cdylib", "lib"]`. It depends on `godot` 0.4.5 (gdext) and `rayon` 1.8 for its thread pool. The GDExtension entry point `GdPlanningAIExt` calls `logger::init_log_channel()` at `InitLevel::Servers`.

Module map of `rust/src/`:

| Module | Role | |--------|------| | `lib.rs` | Crate root, module declarations, GDExtension entry. | | `planner/mod.rs` | `SearchHeuristic` trait, Dijkstra/A\* heuristics, `TerminationStrategy`. | | `planner/engine.rs` | `PlannerEngine`, the search state machine (`step_search`, `process_simulation`). | | `planner/expander.rs` | Candidate discovery (`find_candidates`, `get_discovery_result`). | | `planner/simulation.rs` | `StepResult`/`SimResult`, `eval_precondition`, `simulate_action`. | | `planner/types.rs` | `PlanBranch`, `SearchNode`, `PriorityNode`, `SearchContext`, insertion logic. | | `plan_types.rs` | `ActionSpec`, `GoalSpec`, `PreconditionSpec`, callback channel messages. | | `requirement.rs` | `RequirementSpec`/`ProvisionSpec` enums, matching and holding predicates. | | `scheduler.rs` | `GdPAIPlanScheduler`: job lifecycle, callable registry, dispatch. | | `snapshot.rs` | `VariantSnapshot`, `SimObjectData`, `BlackboardSnapshot` (send-safe mirrors). | | `gdpai_blackboard.rs` | `GdPAIBlackboard` Godot class (agent/world state). | | `precondition.rs` | `PreconditionHandler` (main-thread reconstruction for the pre-filter). | | `plan_tree.rs` | `PlanResult`, the plan handed back to GDScript. | | `sim_object_proxy.rs` | `SimObjectProxy`, a world-object snapshot. | | `debug_tree.rs` | `TreeDump`, search-tree dump structures backing the `get_debug_tree` text output. | | `logger.rs` | Tiered logging macros plus a thread-safe log channel. |

### 3.3 How the two languages communicate

Planning runs on a Rayon worker thread, but GDScript callables can only run on the main thread. The two sides communicate over a request/response channel protocol:

1. `GdPAIRustBridge.serialize_actions()` and `serialize_goals()` turn GDScript `Action`/`Goal` objects into plain `Array[Dictionary]` payloads. The bridge registers the actual callables (`get_action_cost`, `simulate_effect`, the eval callables) into a per-job registry on the Rust side; the serialized specs carry only callable **ids** plus the preconditions, requirements, and provisions as bridge dictionaries.
1. `GdPAIPlanScheduler.submit_plan()` snapshots the blackboards into `BlackboardSnapshot`s, builds a `SearchContext` and a `PlannerEngine`, and spawns the worker.
1. When the worker needs a GDScript callable, it sends a `CallbackRequest { request_id, callable_id, kind, bindings, response_tx }` over the job's request channel and parks the node. `CallbackKind` is one of `GetCost | ApplyEffect | EvalCustomPrecond`.
1. On the main thread, `GdPAIAutoload._process()` calls `scheduler.process_callbacks()` every frame. It recovers finished engines, delivers `Complete` results to `agent._on_plan_ready(dict)`, drains the request channel, looks up each callable in the registry, and dispatches it via `dispatch_callback`. The response goes back on the request's `response_tx` as a `PlannerCallback` (`Float`, `Bool`, or `UpdatedSnapshots`).
1. Bindings are injected into the agent blackboard **only** (`inject_bindings_into_agent`, using `values[0]`). The next `step_search` resume cycle re-queues parked nodes whose responses arrived this frame.

The send-safe `VariantSnapshot` has three tiers: Tier 1 primitives, Tier 2 `var_to_bytes` blobs (vectors, colors, dicts, resources), and Tier 3 live `ObjectRef(i64)` instance handles. Conversion must happen on the main thread; `BlackboardSnapshot` is what actually crosses thread boundaries.

______________________________________________________________________

## 4. Class hierarchy overview

The GDScript classes in `scripts/`, grouped by base type:

### Node classes (scene nodes)

| Class | File | Notes | |-------|------|-------| | `GdPAIAgent` | `scripts/nodes/gdpai_agent.gd` | The central runtime. Two blackboards (self + world), goals, actions, plan execution. | | `GdPAIWorldNode` | `scripts/nodes/gdpai_world_node.gd` | Sources world state; rescans the scene tree for `GdPAIObjectData` nodes. | | `GdPAIObjectData` | `scripts/nodes/gdpai_object_data.gd` | Base for world objects; broadcasts provided actions. | | `GdPAILocationData` | `scripts/nodes/gdpai_location_data.gd` | Extends `GdPAIObjectData`; 2D/3D position and rotation. | | `GdPAIInteractable` | `scripts/nodes/gdpai_interactable.gd` | Extends `GdPAIObjectData`; interaction distance and drift limits. |

### RefCounted extension points

| Class | File | Notes | |-------|------|-------| | `Action` | `scripts/refcounteds/action.gd` | Planning-time overrides (`get_validity_checks`, `get_action_cost`, `get_preconditions`, `get_requirements`, `get_provisions`, `simulate_effect`) and execution-time overrides (`pre/perform/post_perform_action`). | | `GoToAction` | `scripts/refcounteds/goto_action.gd` | Extends `Action`. Wildcard `at_target` provision; handles navigation via `inject_binding`. | | `Goal` | `scripts/refcounteds/goal.gd` | `compute_reward`, `get_desired_state`. | | `PropertyUpdater` | `scripts/refcounteds/property_updater.gd` | `update_properties` every frame, `initialize` once. | | `Precondition` | `scripts/refcounteds/precondition.gd` | Base plus static factories (agent, world state, world object proxy, custom). | | `PreconditionBuiltin` | `scripts/refcounteds/precondition_builtin.gd` | Evaluated natively in Rust; no callback. | | `PreconditionCustom` | `scripts/refcounteds/precondition_custom.gd` | Evaluated via a main-thread callback. | | `PreconditionCustomWithDeps` | `scripts/refcounteds/precondition_custom.gd` variant | Adds `dependent_object_ids` for Rust liveness validation. | | `RequirementSpec` + subclasses | `scripts/refcounteds/requirement_spec*.gd` | `binding_exists`, `binding_equals`, `binding_in_set`, `fact`. | | `ProvisionSpec` + subclasses | `scripts/refcounteds/provision_spec*.gd` | `binding`, `fact`, `fact_wildcard` (the wildcard is how `GoToAction` satisfies any location requirement). |

### Resource classes (config)

| Class | File | Notes | |-------|------|-------| | `GdPAIAgentConfig` | `scripts/resources/gdpai_agent_config.gd` | Planning strategy (CONTINUOUS, ON_INTERVAL, ON_DEMAND, ON_INTERVAL_FORCED), max recursion, iteration budget, blackboard plan, behavior configs. | | `GdPAIBehaviorConfig` | `scripts/resources/gdpai_behavior_config.gd` | Groups goals, self actions, and property updaters into reusable behavior modules. | | `GdPAIBlackboardPlan` | `scripts/gdpai_blackboard_plan.gd` | Template for agent blackboard properties; `generate_blackboard()`. |

### Rust-exposed classes (no `.gd` file)

- `GdPAIBlackboard`: `set/get/erase/has_property`, `set_dict`, `get_object_for` (returns a `SimObjectProxy` snapshot), group queries. The reserved `GDPAI_OBJECTS` key holds arrays of `GdPAIObjectData` nodes.
- `SimObjectProxy`: a send-safe world-object snapshot (uid, groups, properties).
- `GdPAIPlanScheduler`: the planning job manager and the callable dispatcher.

### Internal surface (do not use)

`Action.set_state/get_state/erase_state/has_state` (namespaced state on the agent blackboard), all `GdPAIAgent` underscore members, `GdPAIRustBridge` (the agent owns it), and the base `to_bridge_dict` implementations.

______________________________________________________________________

## 5. Runtime flow

The agent lifecycle, from configuration to execution:

1. **Config to blackboard**: `GdPAIAgent._ready()` builds the agent blackboard from `config.blackboard_plan`, sets `entity` and the `GDPAI_OBJECTS` entry (children in the `GdPAIObjectData` group under the entity), applies each behavior config via `apply_to_agent` (which appends goals/self actions and calls `initialize` on property updaters), finds the world node, and creates the Rust bridge.
1. **Per-frame updates**: `_process(delta)` runs property updaters (hunger decay, stamina regen), checks the planning strategy, and drives plan execution.
1. **Plan submit**: `_start_plan_async()` refreshes `GDPAI_OBJECTS`, collects `self_actions + world-provided actions`, bumps a generation counter, and calls `scheduler.submit_plan(...)` with the serialized actions/goals and the config limits (`max_recursion`, `iteration_budget`). The search runs on a background thread.
1. **Plan ready**: `_on_plan_ready(result)` is generation-guarded so stale results from a superseded submission are discarded. It deserializes the plan into `{action_chain, bindings_by_position}`, sets the current goal from `result.goal_index`, and clones the actions via `clone_for_plan()`.
1. **Execution**: plans run in three phases. **Pre**: every action's `pre_perform_action` runs; any FAILURE aborts the plan. **Action**: the current action's `perform_action(agent, delta)` returns SUCCESS (advance), RUNNING (stay), or FAILURE (abort). **Post**: every action's `post_perform_action` runs as guaranteed cleanup.

Planning strategies: CONTINUOUS replans each frame once the current plan is done; ON_INTERVAL replans on a timer; ON_INTERVAL_FORCED replans every tick regardless of plan state; ON_DEMAND only plans when `manually_start_plan()` is called.

______________________________________________________________________

## 6. Build system

Prefer `make` targets over ad hoc commands. Two Makefiles exist.

### Root `Makefile`

| Target | Purpose | |--------|---------| | `test`, `test-rust`, `test-godot` | Run all / Rust / Godot suites. `test` runs both and exits nonzero if any sub-suite fails. `test-godot` exits 0 with a `GUT-SUITE-OK`/`GUT-SUITE-FAILED` verdict line (checks load/parse error markers + gut exit status); `test-rust` propagates cargo test's exit code with a `CARGO-TEST-OK`/`CARGO-TEST-FAILED` verdict. | | `test-godot-pipe-output` | Full GUT log written to `test_output.log`. | | `format`, `format-rust`, `format-godot` | Format all / Rust (rustfmt) / GDScript (`fix_gd_spacing.py` + gdformat via uv). | | `lint-style` | Run `uv run scripts/lint_style.py`. | | `check-docs`, `sync-docs` | Diff/sync README + LICENSE between the repo root and the addon copy. | | `launch-editor`, `addons-install`, `godot-pin`, `godot-install-pinned` | Editor and Godot version management. | | `record-obs`, `record-obs-fullscreen` | Scene recording via OBS (SCENE/DURATION/FPS/OUTPUT/FULLSCREEN vars). |

### `addons/GdPlanningAI/rust/Makefile`

| Target | Purpose | |--------|---------| | `build-release` | `cargo build --release`, then copies the shared library to `addons/GdPlanningAI/bin/{linux,macos,windows}/`. | | `build-debug`, `build-all`, `clean-binaries` | Other build variants and cleanup. | | `build-linux`, `build-windows` | Cross-compilation (uses `cross`, needs Docker/Podman). | | `test` | `cargo test`. | | `format`, `lint`, `lint-fix` | rustfmt, clippy (errors block), clippy autofix. | | `clean`, `doc` | Cargo clean and `cargo doc`. | | `setup-cross` | Install `cross` for cross-compiling. |

### Build artifact flow

`make build-release` produces the shared library and copies it into `addons/GdPlanningAI/bin/<platform>/`. `gdplanningai.gdextension` loads the platform-appropriate file at runtime (`.so` on Linux, `.dll` on Windows, `.dylib` on macOS). The binaries are committed to the repo.

Important: after any Rust change you must run `make build-release` before the GDScript test suite will pick up the new behavior.

______________________________________________________________________

## 7. Testing

Two independent suites.

### GDScript suite (GUT)

Configured by `.gutconfig.json` (`dirs ["res://test"]`, `include_subdirs true`). Run with `make test-godot` (headless GUT via `addons/gut/gut_cmdln.gd`).

- `test/unit/` (2 files): `test_sim_object_proxy.gd` (SimObjectProxy property ops) and `test_gdpai_blackboard.gd` (blackboard property ops, dict round-trips, `GDPAI_OBJECTS` to proxies, group queries, clone isolation).
- `test/integration/` (5 files): blackboard clone isolation; async planner behavior (empty plans, satisfied goals, cheapest plan selection, cancellation of stale submissions); requirements/provisions chaining (Pickup to Eat, GoTo wildcard chaining, binding sets, cost comparisons); campfire example smoke tests against the real `campfire_2d.tscn`; and the hunger example smoke test (a full wander, shake, pick-up, eat cycle).

### Rust suite

In `addons/GdPlanningAI/rust/tests/` (11 files plus `common/mod.rs` helpers). Run with `make test-rust` (`cargo test`). Integration tests build `PlannerEngine`/`SearchContext` directly with mock callback-responder threads, so no Godot runtime is needed. They cover the search state machine (goal priority, depth, budget, cancellation), BestCost vs FirstComplete termination, requirement/provision chaining, branch insertion, snapshot evaluation, debug tree rendering, and a regression test for the Verifying state.

______________________________________________________________________

## 8. Developer tooling

- **Formatting and linting**: `make format-rust` (rustfmt), `make format-godot` (spacing fixer + gdformat), `make lint-style` (style linter over git-tracked `.gd`/`.rs` files: bans `:=`, checks borders, docstrings, and spacing).
- **`scripts/` one-liners**: `fix_gd_spacing.py` (2 blank lines before top-level funcs), `lint_style.py` (style linter), `capture_obs.py` + `obs_controller.py` (OBS WebSocket recording), `capture_all_showcase.sh` (sequential showcase recording).
- **Pre-commit hooks** (`.pre-commit-config.yaml`, pre-commit + pre-push): pre-commit-hooks v5.0.0, cargo fmt/clippy (via the rust Makefile), gitleaks v8.24.0, mdformat 0.7.19 (`--wrap=80`), ruff v0.6.0 + ruff-format, gdformat + gdlint-style, and `check-docs`.
- **Third-party addons** (`addons.jsonc`): `gut` v9.6.0 (bitwes/Gut), installed via GodotEnv, gitignored, cached under `.addons/`. Both editor plugins (GdPlanningAI and gut) are enabled.
- **`project.godot`**: Godot 4.7, Forward Plus renderer, **no main scene** (run scenes directly). Autoload: `GdPAIAutoload`. There is a vestigial `.NET`/Mono section that is unused.
- **No CI**: there is no `.github/` and no GitHub Actions. Quality gates are the local Make targets and the pre-commit/pre-push hooks.

______________________________________________________________________

## 9. Documentation map

| Doc | Purpose | |-----|---------| | `README.md` | Project pitch: what GdPlanningAI is, GOAP background, installation, usage concepts, demo list, FAQ. | | `docs/CODEBASE_OVERVIEW.md` | This file: the map of the whole repo. | | `docs/PLANNER_PSEUDOCODE.md` | High-level pseudocode for the planning search. | | `docs/ALGORITHM.md` | Deep dive into the Rust engine's search algorithm, built from the Rust source. | | `docs/AUTHORING_GUIDE.md` | How to write actions, goals, preconditions, and object data for your own agents. | | `docs/EXAMPLES.md` | Step-by-step walkthroughs of the example scenes. | | `docs/DOCUMENTATION_GUIDELINES.md` | Code documentation style: no decorative headers, docstring conventions for GDScript and Rust. |

The `notes/` folder is for session-scoped research (for example, `RESEARCH_RUST_ENGINE.md`, `RESEARCH_GDSCRIPT_API.md`, `RESEARCH_TESTS_TOOLING.md`). Treat it as scratch: it documents the current state of the code at a point in time and is expected to go stale. Reference docs live in `docs/`.

A good reading order for a new contributor: this file, then `docs/ALGORITHM.md`, then `docs/EXAMPLES.md` for a concrete end-to-end picture.
