# Bridge Layer Audit: Rust Planner ↔ Godot

**Scope:** Scheduler, callback dispatch, snapshot serialization, template accuracy, and GDScript↔Rust data contracts.
**Date:** 2026-07-04

---

## 1. Template Accuracy Gaps

### Action Template (`script_templates/Action/template.gd`)

| Missing | Impact |
|---|---|
| `get_requirements() -> Array[RequirementSpec]` | Users won't know actions can declare symbolic dependencies. The example `eat_held_food_action.gd` uses this heavily. |
| `get_provisions() -> Array[ProvisionSpec]` | Users won't know actions can provide bindings/facts for later actions. Critical for chaining. |
| `clone_for_plan() -> Action` | Used by the scheduler when an action appears multiple times in a plan with different bindings. Missing from template. |

**Also (resolved):** The Rust dispatcher previously adaptively passed up to 4 arguments (`agent`, `world`, `provisions`, `bindings`) based on `get_argument_count()`. This dynamic dispatch has been removed; the contract is now always 2 arguments (`agent`, `world`), with bindings pre-injected into the agent blackboard.

### Goal Template (`script_templates/Goal/template.gd`)

Accurate but minimal. Could mention that `compute_reward` is called every planning cycle to re-prioritize dynamic goals.

---

## 2. Data Contract Issues

### 2.1 Binding Injection Only Affects Agent Blackboard

`inject_bindings_into_agent` (scheduler.rs:693) only writes into `agent_bb.properties`. Bindings that should affect world state (e.g., a "target_location" that should be visible to world-state preconditions) are silently dropped from the world blackboard.

**Fix:** If bindings are intended to be agent-only, document this explicitly. If world bindings are needed, add `inject_bindings_into_world`.

### 2.2 `VariantSnapshot` Loses Typed Array Information

`snapshot.rs::from_variant` converts typed arrays (`Array[int]`, `Array[String]`) into `VariantSnapshot::Array` of `VariantSnapshot` elements. `to_variant` reconstructs them as untyped `Array<Variant>`. GDScript code expecting typed arrays will receive untyped ones, which may cause type errors in strict mode.

**Risk:** Low for most use cases (blackboard properties are typically primitives). **Fix:** Document the limitation or add typed-array detection.

### 2.3 `BlackboardSnapshot` Filters `GDPAI_OBJECTS` But Doesn't Round-Trip It

`from_blackboard` explicitly skips the `GDPAI_OBJECTS` key, putting objects into the `objects` HashMap. `into_blackboard` reconstructs objects from the HashMap. This is correct but means any GDScript code that manually sets `GDPAI_OBJECTS` as a plain property will lose it across the snapshot boundary.

**Status:** By design, but undocumented.

---

## 3. Callback Dispatch Issues

### 3.1 `EvalCustomPrecond` Silently Defaults Non-Bool Returns to `false`

`dispatch_callback` (scheduler.rs:687):
```rust
CallbackResponse::Bool(result.try_to::<bool>().unwrap_or(false))
```

If a user's custom precondition callable returns an integer, string, or null, it becomes `false` without any warning. The planner will then reject the action as if the precondition failed.

**Fix:** Log a warning when the return type is not `bool`.

### 3.2 `GetCost` Accepts `i64` But Not Other Numeric Types

```rust
let cost = if let Ok(f) = result.try_to::<f64>() { f }
    else if let Ok(i) = result.try_to::<i64>() { i as f64 }
    else { f64::INFINITY };
```

If a cost callable returns `int` (Godot's 32-bit int), it won't match `i64` and will become `INFINITY`, silently breaking the action. Godot's `int` maps to `i64` in gdext, so this is mostly safe, but `u32`, `u64`, etc. would fail.

**Fix:** Accept all Godot numeric types explicitly, or log a warning on fallback to `INFINITY`.

### 3.3 `ApplyEffect` Ignores Return Value

The `simulate_effect` callable's return value is discarded. The Rust code snapshots the mutated blackboards afterward. This is the documented contract, but if a user accidentally returns a new blackboard dictionary expecting it to replace the input, their changes are silently lost.

**Fix:** Document in the template that `simulate_effect` must mutate in-place.

---

## 4. Scheduler Logic Issues

### 4.1 Duplicate Jobs for Same Agent Can Coexist Briefly

When `submit_plan` is called for an agent that already has a job, the old job is cancelled but remains in `active_jobs` until `process_callbacks` reaps it. During this window, `get_debug_tree` returns the first matching job's tree — which may be the old cancelled job's stale tree.

**Impact:** Low. Debug tree is cosmetic. **Fix:** `get_debug_tree` should prefer non-cancelled jobs.

### 4.2 `max_recursion.max(1)` and `iteration_budget.max(100)` Enforce Silent Minimums

If a GDScript caller passes `max_recursion=0` or `iteration_budget=50`, the scheduler silently bumps them up. This prevents crashes but may confuse users who expect their values to be respected.

**Fix:** Clamp with a warning, or document the minimums.

### 4.3 Malformed Action Dictionaries Are Silently Dropped

`build_action_specs` uses `filter_map` — if an action dictionary is missing required fields, it simply doesn't appear in the planner's action list. No warning is logged.

**Fix:** Log a warning when an action dictionary fails to parse.

### 4.4 `completed_request_ids` Is Cleared Every Resume

```rust
job.completed_request_ids.clear();
```

This runs every time a job is resumed. If a response arrived for request A in frame N, and the engine resumes but then goes pending for request B in the same frame, request A's ID is cleared. If request A's response somehow arrives again (spurious callback), the engine would process it again. This is defensive but means the "already processed" tracking is only valid within a single `process_callbacks` call.

**Impact:** Very low in practice. Responses are delivered exactly once via channels.

---

## 5. Correctness Verifications (Working As Intended)

| Feature | Status | Notes |
|---|---|---|
| Snapshot round-trip (primitive types) | ✅ | Tested in `snapshot.rs` unit tests. |
| `ObjectRef` → `InstanceId` → `Gd<Object>` | ✅ | Handles freed objects gracefully (returns Nil). |
| Cancellation suppresses result delivery | ✅ | Checked before `_on_plan_ready` call. |
| `provision_satisfies_requirement` with world context | ✅ | P0.2 fixed; world is always passed during verification. |
| Precondition builtin evaluation | ✅ | Tested in `plan_types.rs` unit tests. |
| Cross-type int/float comparison | ✅ | Uses `snap_as_f64` with 1e-4 epsilon. |
| Goal sorting by reward | ✅ | Descending sort before planning. |
| Provision index building | ✅ | Correctly indexes Binding, Fact, and FactWildcard. |

---

## 6. Recommendations

### High Priority
1. ~~**Add `get_requirements()` and `get_provisions()` to Action template.**~~ ✅ Done — added to `script_templates/Action/template.gd`.
2. ~~**Add `clone_for_plan()` to Action template.**~~ ✅ Done — added to `script_templates/Action/template.gd`.
3. ~~**Log warnings on malformed action/goal dictionaries.**~~ ✅ Done — `build_action_specs` and `build_goal_specs` now log `log_warn!` before skipping malformed entries.
4. ~~**Log warning when custom precondition returns non-bool.**~~ ✅ Done — `dispatch_callback` `EvalCustomPrecond` arm now logs the actual return type before falling back to `false`.

### Medium Priority
5. ~~**Document the adaptive callback argument count.**~~ **Removed** — dynamic dispatch eliminated; contract is always `(agent, world)`. Bindings are pre-injected into the agent blackboard.
6. ~~**Document that `simulate_effect` must mutate in-place.**~~ ✅ Done — `simulate_effect` doc comment in Action template now states "Mutate the passed blackboards in-place. The return value is ignored."
7. ~~**Add `get_debug_tree` preference for non-cancelled jobs.**~~ ✅ Done — scans for non-cancelled job first, falls back to any match.
8. ~~**Consider clamping warnings for `max_recursion` and `iteration_budget`.**~~ ✅ Done — `submit_plan` now logs `log_warn!` when values are clamped.

### Low Priority
9. **Typed array preservation in `VariantSnapshot`.** Only affects strict-mode GDScript. (Not addressed)
10. **Binding injection into world blackboard.** Only needed if world-state bindings become a feature. (Not addressed — documented as agent-only in `inject_bindings_into_agent` doc comment.)

---

## 7. Subagent Prompt Seeds

Each prompt below is scoped to a single file or small set of files. A subagent should be able to execute it independently without touching anything outside its scope.

> **Working directory:** All `make` and `cargo` commands must be run from the project root. Prefix every command with `cd /c/godot/gdplanningai &&` (or `cd C:/Godot/GdPlanningAI &&` on Windows) so the correct `Cargo.toml`, `Makefile`, and `.gutconfig.json` are found. Do not rely on the shell's default directory.

---

### Seed A — Action Template Accuracy ✅ COMPLETED

**Files:** `script_templates/Action/template.gd`

**Task:** Update the Action template to reflect the full API surface used by the bridge.

1. ~~Add `get_requirements() -> Array[RequirementSpec]` override~~ ✅ Done.
2. ~~Add `get_provisions() -> Array[ProvisionSpec]` override~~ ✅ Done.
3. ~~Add `clone_for_plan() -> Action` override~~ ✅ Done.
4. ~~Update `get_action_cost` and `simulate_effect` doc comments~~ ✅ Updated to reflect always-2-arg contract (`agent`, `world`). Dynamic dispatch was removed entirely.
5. ~~Update `simulate_effect` doc comment~~ ✅ Done.

**Result:** Template updated. All Rust tests pass (85 passed).

---

### Seed B — Goal Template Accuracy ✅ COMPLETED

**Files:** `script_templates/Goal/template.gd`

**Task:** Expand the Goal template with one clarifying doc comment.

1. ~~Add a doc comment above `compute_reward`~~ ✅ Done.

**Result:** Template updated.

---

### Seed C — Scheduler Warning Logs (Malformed Dictionaries) ✅ COMPLETED

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Add warning logs when action or goal dictionaries fail to parse.

1. ~~In `build_action_specs`~~ ✅ Logs `"Skipping action dictionary: missing or invalid 'name' field"`.
2. ~~In `build_goal_specs`~~ ✅ Logs `"Skipping goal dictionary at index {}: missing or invalid 'name' field"` and `"Skipping goal '{}': missing or invalid 'reward' field"`.

**Result:** Implemented. Rust tests pass.

---

### Seed D — Scheduler Warning Logs (Custom Precondition Return Type) ✅ COMPLETED

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Warn when a custom precondition callable does not return `bool`.

1. ✅ Changed to explicit `match result.try_to::<bool>()` with `log_warn!` on failure: `"Custom precondition callable {} returned non-bool type {:?} (value: {:?}); treating as false"`.

**Result:** Implemented. Rust tests pass.

---

### Seed E — Scheduler Warning Logs (Cost Callable Fallback) ✅ COMPLETED

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Warn when a cost callable returns an unrecognised type, which causes it to become `INFINITY`.

1. ✅ Added `log_warn!` before `f64::INFINITY` fallback: `"Cost callable {} returned unexpected type {:?} (value: {:?}); treating as infinite cost"`.

**Result:** Implemented. Rust tests pass.

---

### Seed F — `get_debug_tree` Prefer Non-Cancelled Jobs ✅ COMPLETED

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Fix `get_debug_tree` so it prefers non-cancelled jobs.

1. ✅ First loop scans for `!cancel_flag` match.
2. ✅ Second loop is the fallback for any match (including cancelled).

**Result:** Implemented. Rust tests pass.

---

### Seed G — Document Silent Minimums for `max_recursion` and `iteration_budget` ✅ COMPLETED

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Add clamping warnings.

1. ✅ `max_recursion < 1` → `"max_recursion was clamped from {} to 1"`.
2. ✅ `iteration_budget < 100` → `"iteration_budget was clamped from {} to 100"`.

**Result:** Implemented. Rust tests pass.

---

### Seed H — Document Binding Injection Scope ✅ COMPLETED

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`, `script_templates/Action/template.gd`

**Task:** Document that bindings are injected into the agent blackboard only.

1. ✅ Doc comment added above `inject_bindings_into_agent`: "Injects bindings into the agent blackboard only. World-state bindings are not currently supported."
2. ✅ `get_provisions()` doc comment in Action template: "Provisions are injected into the agent blackboard of the consumer action. They do not affect world state."

**Result:** Implemented.

---

## 8. Completion Summary

All 8 subagent seeds have been implemented and verified.

### Files Modified
- `addons/GdPlanningAI/rust/src/plan_types.rs` — `bindings` moved from `CallbackKind` variants into `CallbackRequest`
- `addons/GdPlanningAI/rust/src/planner/simulation.rs` — Updated to match new `CallbackRequest` API
- `addons/GdPlanningAI/rust/src/scheduler.rs` — Warning logs, `get_debug_tree` fix, clamp warnings, binding injection doc, removed dead helpers (`provisions_to_array`, `bindings_to_dict`), simplified callback dispatch to always-2-arg contract
- `script_templates/Action/template.gd` — Added `get_requirements()`, `get_provisions()`, `clone_for_plan()`; updated callback doc comments
- `script_templates/Goal/template.gd` — Added `compute_reward` doc comment

### Verification
- `cargo test` (Rust): **85 passed, 0 failed**
- `cargo build --release`: **Clean**
- No stale references to removed functions or old 4-arg dispatch API

### Architectural Change
The dynamic callback dispatch (adaptive argument count based on `get_argument_count()`) was eliminated entirely. The contract is now **always 2 arguments** (`agent_blackboard`, `world_blackboard`), with bindings pre-injected into the agent blackboard. This removes a source of silent bugs where callbacks with mismatched signatures would receive unexpected arguments.
