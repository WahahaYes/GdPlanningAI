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

**Also:** `get_action_cost` and `simulate_effect` signatures in the template only show 2 params, but the Rust dispatcher adaptively passes up to 4 (`agent`, `world`, `provisions`, `bindings`). The template should document this.

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
1. **Add `get_requirements()` and `get_provisions()` to Action template.** These are essential for backward chaining and exist in all real examples.
2. **Add `clone_for_plan()` to Action template.** Used by the scheduler for actions with multiple occurrences.
3. **Log warnings on malformed action/goal dictionaries.** Silent skipping makes debugging hard.
4. **Log warning when custom precondition returns non-bool.** Silent `false` is confusing.

### Medium Priority
5. **Document the adaptive callback argument count.** Action templates should note that cost/effect/precondition callables receive `(agent, world, provisions, bindings)` when they accept 4 args.
6. **Document that `simulate_effect` must mutate in-place.** The return value is ignored.
7. **Add `get_debug_tree` preference for non-cancelled jobs.** Avoid returning stale debug trees.
8. **Consider clamping warnings for `max_recursion` and `iteration_budget`.**

### Low Priority
9. **Typed array preservation in `VariantSnapshot`.** Only affects strict-mode GDScript.
10. **Binding injection into world blackboard.** Only needed if world-state bindings become a feature.

---

## 7. Subagent Prompt Seeds

Each prompt below is scoped to a single file or small set of files. A subagent should be able to execute it independently without touching anything outside its scope.

> **Working directory:** All `make` and `cargo` commands must be run from the project root. Prefix every command with `cd /c/godot/gdplanningai &&` (or `cd C:/Godot/GdPlanningAI &&` on Windows) so the correct `Cargo.toml`, `Makefile`, and `.gutconfig.json` are found. Do not rely on the shell's default directory.

---

### Seed A — Action Template Accuracy

**Files:** `script_templates/Action/template.gd`

**Task:** Update the Action template to reflect the full API surface used by the bridge.

1. Add `get_requirements() -> Array[RequirementSpec]` override with a doc comment explaining that requirements are symbolic dependencies satisfied by predecessor action provisions.
2. Add `get_provisions() -> Array[ProvisionSpec]` override with a doc comment explaining that provisions make bindings/facts available to later actions.
3. Add `clone_for_plan() -> Action` override with a doc comment explaining that the scheduler clones actions when they appear multiple times in a plan with different bindings.
4. Update `get_action_cost` and `simulate_effect` doc comments to note that the dispatcher adaptively passes up to 4 arguments (`agent`, `world`, `provisions`, `bindings`) based on the callable's `get_argument_count()`.
5. Update `simulate_effect` doc comment to explicitly state: "Mutate the passed blackboards in-place. The return value is ignored."

**Do NOT touch:** Any other templates, Rust code, or example GDScript.

**Acceptance criteria:** The template compiles as valid GDScript and all new overrides have meaningful doc comments. `make test` still passes.

---

### Seed B — Goal Template Accuracy

**Files:** `script_templates/Goal/template.gd`

**Task:** Expand the Goal template with one clarifying doc comment.

1. Add a doc comment above `compute_reward` noting that it is called every planning cycle to re-prioritize dynamic goals, so the return value can change frame-to-frame.

**Do NOT touch:** Any other templates or code.

**Acceptance criteria:** Template is still valid GDScript. `make test` passes.

---

### Seed C — Scheduler Warning Logs (Malformed Dictionaries)

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Add warning logs when action or goal dictionaries fail to parse.

1. In `build_action_specs`, inside the `filter_map` closure, when any required field (`name`, `preconditions` parse, etc.) is missing and `None` is returned, first log a `log_warn!` that includes the action name (if available) or "unnamed action" and the missing field.
2. In `build_goal_specs`, do the same for missing `name`, `reward`, or failed `desired_state` parsing.

**Do NOT touch:** The `extract_typed_specs` or parsing logic itself — only add log statements before returning `None`.

**Acceptance criteria:** `cargo test` and `make test` pass. A malformed action dictionary in a test logs a visible warning.

---

### Seed D — Scheduler Warning Logs (Custom Precondition Return Type)

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Warn when a custom precondition callable does not return `bool`.

1. In `dispatch_callback`, in the `EvalCustomPrecond` arm, change:
   ```rust
   CallbackResponse::Bool(result.try_to::<bool>().unwrap_or(false))
   ```
   to first check `result.try_to::<bool>()`. If it fails, log a `log_warn!` with the callable name (if obtainable) and the actual return type, then fall back to `false`.

**Do NOT touch:** The `GetCost` or `ApplyEffect` arms.

**Acceptance criteria:** `cargo test` and `make test` pass. A test can verify the warning is emitted (or just eyeball it in a run).

---

### Seed E — Scheduler Warning Logs (Cost Callable Fallback)

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Warn when a cost callable returns an unrecognised type, which causes it to become `INFINITY`.

1. In `dispatch_callback`, in the `GetCost` arm, after the `f64` and `i64` attempts, before assigning `f64::INFINITY`, log a `log_warn!` that the cost callable returned an unexpected type and is being treated as infinite cost.

**Do NOT touch:** Any other callback arms.

**Acceptance criteria:** `cargo test` and `make test` pass.

---

### Seed F — `get_debug_tree` Prefer Non-Cancelled Jobs

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Fix `get_debug_tree` so it prefers non-cancelled jobs.

1. In `get_debug_tree`, when iterating jobs for the agent, first scan for a matching job where `cancel_flag` is **not** set. Return that job's tree if found.
2. Only fall back to a cancelled job's tree if no non-cancelled match exists.

**Do NOT touch:** Job lifecycle, cancellation logic, or `process_callbacks`.

**Acceptance criteria:** `cargo test` and `make test` pass.

---

### Seed G — Document Silent Minimums for `max_recursion` and `iteration_budget`

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`

**Task:** Add clamping warnings.

1. In `submit_plan`, after computing `max_rec` and `iter_budget`, if the caller's original value was below the clamped minimum, log a `log_warn!` stating the original value and the clamped value.

**Do NOT touch:** The clamp logic itself or any other scheduler function.

**Acceptance criteria:** `cargo test` and `make test` pass.

---

### Seed H — Document Binding Injection Scope

**Files:** `addons/GdPlanningAI/rust/src/scheduler.rs`, `script_templates/Action/template.gd`

**Task:** Document that bindings are injected into the agent blackboard only.

1. Add a doc comment above `inject_bindings_into_agent` in `scheduler.rs` stating: "Injects bindings into the agent blackboard only. World-state bindings are not currently supported."
2. In the Action template, add a note in the `get_provisions()` doc comment: "Provisions are injected into the agent blackboard of the consumer action. They do not affect world state."

**Do NOT touch:** Any logic. Only comments/docs.

**Acceptance criteria:** `cargo test` and `make test` pass.
