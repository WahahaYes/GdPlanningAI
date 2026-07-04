# Rust Planner Algorithm Audit — Source of Truth

**Date:** 2026-07-03
**Scope:** `addons/GdPlanningAI/rust/src/` — core backward-chaining planner, scheduler, snapshot, precondition, and requirement systems.
**Status:** Audit complete. Findings categorized by severity. Ready for subagent delegation.

---

## Executive Summary

The Rust planner implements a hybrid backward-chaining GOAP search but suffers from two critical logic bugs, extensive dead/obsolete code, stale documentation, and maintainability debt. All findings below are independently actionable and scoped for subagent work.

---

## Severity Legend

- **P0 — Critical:** Logic bug that can produce incorrect plans or silently accept invalid chains.
- **P1 — High:** Dead/obsolete code or documentation drift that misleads future maintainers.
- **P2 — Medium:** Maintainability debt; refactoring or cleanup.
- **P3 — Low:** Cosmetic, naming, or purely decorative items.

---

## P0 — Critical Logic Bugs

### P0.1 Post-Action State Checked Against Action's Own Preconditions

- **Location:** `src/planner/engine.rs`, lines 508–526
- **Root Cause:** After discovery-simulating a candidate action, the code removes preconditions at `pos == 0` (the newly prepended action's own preconditions) if they happen to be satisfied by the **post-action** discovery state (`disc_res.agent`, `disc_res.world`). Preconditions must hold *before* an action runs, not after.
- **Impact:** Can incorrectly prune valid preconditions or allow invalid plans where an action satisfies its own precondition via its own effect.
- **Fix Guidance:** Remove the block entirely, or if the intent was to use the discovery simulation to show the action's preconditions are already met by the *initial* state, check against `ctx.initial_agent`/`ctx.initial_world` instead. Given this same check is already done at lines 441–450 (skipping preconditions already met by initial state), the block is redundant and should be deleted.
- **Subagent Scope:** Single-file edit in `engine.rs`. Add a regression Rust test proving the bug and its fix.

### P0.2 `BindingInSet` Requirements Weakened During Forward Validation

- **Location:** `src/planner/engine.rs`, lines 706–712
- **Root Cause:** `process_simulation` calls `provision_satisfies_requirement(prov, req, None)` with `world = None`. In `src/requirement.rs:317–336`, `BindingInSet` falls back to a simple name match when `world` is `None`, completely ignoring the group-membership constraint.
- **Impact:** A `BindingInSet { binding_name: "held_item", set_name: "food" }` requirement is treated as `BindingExists { binding_name: "held_item" }` during forward validation. A non-food item could satisfy the requirement.
- **Fix Guidance:** Pass `&branch.current_world` (or `&self.ctx.initial_world` at simulation_index 0) as the `world` argument in `provision_satisfies_requirement` during forward validation.
- **Subagent Scope:** Two-file edit (`engine.rs`, `requirement.rs` if any signature change needed). Add a Rust integration test for `BindingInSet` forward validation.

---

## P1 — High-Priority Stale Code & Documentation

### P1.1 Algorithm Mischaracterization in Crate-Level Docs ✅ FIXED

- **Locations:**
  - `src/lib.rs:3` — "forward-chaining GOAP" → "backward-chaining GOAP"
  - `src/planner/engine.rs:7` — "A* search algorithm" → "backward-chaining Dijkstra search"
  - `src/planner/engine.rs:27` — "A* search queue" → "backward-chaining search queue"
- **Root Cause:** The implementation is backward-chaining Dijkstra (priority = `branch.cost`, no heuristic). `SearchAlgorithm::AStar` and `::DepthFirst` were declared but never produced different behavior.
- **Fix Applied (2026-07-03):**
  1. Replaced `SearchAlgorithm` enum with `SearchHeuristic` trait in `planner/mod.rs`.
  2. Implemented `DijkstraHeuristic` (g-cost priority, `prune_threshold_met`) and `AStarHeuristic` placeholder (h=0, ready for future admissible heuristic).
  3. Introduced `PriorityNode` wrapper in `planner/types.rs` that decouples `BinaryHeap` ordering from the heuristic object.
  4. Added `PlannerEngine::enqueue()` helper that computes priority via the active heuristic on every push.
  5. Updated `scheduler.rs` and all Rust tests to use `with_heuristic(Box::new(DijkstraHeuristic))`.
  6. Corrected doc comments in `lib.rs` and `engine.rs`.
- **Status:** Complete. The trait architecture is now modular — new heuristics can be plugged in without touching the search loop.

### P1.2 Dead Types & Unused Structs ✅ FIXED

| Item | Location | Why Dead | Fix |
|------|----------|----------|-----|
| `SearchAlgorithm::AStar` | `planner/mod.rs:19-23` | Priority is always `cost` | Remove enum or keep only Dijkstra |
| `SearchAlgorithm::DepthFirst` | `planner/mod.rs:19-23` | Same | Same |
| `RipplePolicy` | `plan_types.rs:214-220` | Never referenced anywhere | Deleted |
| `PlanFingerprint` | `plan_tree.rs:23-27` | `PlanBranch::fingerprint()` returns `u64` directly | Deleted |
| `RequestKind` | `plan_types.rs:247-252` | Never referenced | Deleted |
| `SearchTree` | `debug_tree.rs:22-27` | `TreeDump` formats directly, never builds a `SearchTree` | Deleted; `TreeDump` doc comment updated |
| `BranchState::Initializing` | `planner/types.rs:15-16` | Transitions to `Searching` immediately in `process_simulation` with no side effects | Deleted; initial-state precondition filtering moved into `initialize_goal()` |

- **Subagent Scope:** Multi-file dead-code removal. `cargo check` and `cargo test` pass. Note: `SearchAlgorithm` variants were already removed in P1.1.

### P1.3 `to_string()` Shadows `std::string::ToString` ✅ FIXED

- **Locations:** `src/plan_types.rs:95` (`PreconditionSpec::to_string`), `src/requirement.rs:98` (`RequirementSpec::to_string`)
- **Root Cause:** These were hand-written `to_string()` methods, not `std::fmt::Display` impls. They worked but were idiomatically wrong and could silently bypass the standard trait.
- **Fix Applied:** Replaced both with `impl std::fmt::Display`. Call sites in `engine.rs` continue to work via the blanket `ToString` impl.
- **Subagent Scope:** `plan_types.rs`, `requirement.rs`, `engine.rs` call sites.

---

## P2 — Medium: Maintainability & Refactoring

### P2.1 Duplicated Binding-Injection Logic in Scheduler ✅ FIXED

- **Location:** `src/scheduler.rs`, lines 652–658, 691–697, 725–731
- **Root Cause:** All three `CallbackKind` arms in `dispatch_callback` contained identical blocks that injected bindings into `bb_agent.properties`.
- **Fix Applied:** Extracted `inject_bindings_into_agent(agent_bb: &mut Gd<GdPAIBlackboard>, bindings: &[(String, Vec<VariantSnapshot>)])` in `scheduler.rs`. All three arms now call this helper.
- **Subagent Scope:** `scheduler.rs` only.

### P2.2 Nearly-Identical Dictionary-Extraction Functions ✅ FIXED

- **Location:** `src/scheduler.rs`, lines 531–602
- **Root Cause:** `extract_precond_specs`, `extract_requirement_specs`, and `extract_provision_specs` shared the same `try_to::<Array<VarDictionary>>` / `try_to::<VarArray>` fallback pattern and differed only in the element parser.
- **Fix Applied:** Introduced generic `extract_typed_specs<T, F>(dict, key, parse_fn) -> Vec<T>` in `scheduler.rs`. The three old functions are now thin wrappers delegating to it.
- **Subagent Scope:** `scheduler.rs` only.

### P2.3 Manual Index Shifting on Action Prepend ✅ FIXED

- **Location:** `src/planner/engine.rs`, lines 343–352
- **Root Cause:** Three separate loops incremented positions in `open_preconditions`, `open_requirements`, and `action_bindings` when an action was prepended.
- **Fix Applied:** Added `PlanBranch::shift_positions(delta: usize)` in `planner/types.rs`. The engine now calls `new_branch.shift_positions(1)` instead of the three loops.
- **Subagent Scope:** `planner/types.rs` + `planner/engine.rs`.

### P2.4 Manual Index-Compensated Removal Pattern (×2) ✅ FIXED

- **Location:** `src/planner/engine.rs`, lines 404–413 (requirements) and 418–427 (preconditions)
- **Root Cause:** Nearly identical while-loops with `removed_count` adjusted indices when removing by a `HashSet<usize>`.
- **Fix Applied:** Extracted generic `remove_indices<T>(vec: &mut Vec<T>, indices: &HashSet<usize>)` in `planner/engine.rs`. It sorts indices descending before removal, which is cleaner and idiomatically correct. Both call sites now use this helper.
- **Subagent Scope:** `planner/engine.rs` only.

### P2.5 Cost-Caching via Mutable Slice Side Channel

- **Location:** `src/planner/simulation.rs`, lines 82–91
- **Root Cause:** `simulate_action` mutates `branch_action_costs[simulation_index]` through a mutable slice in `SimArgs`, creating a hidden caching channel.
- **Fix Guidance:** Document the pattern explicitly, or refactor so the caller owns caching logic and `simulate_action` returns `(SimResult, Option<f64>)`.
- **Subagent Scope:** `planner/simulation.rs` + `planner/engine.rs` callers.

### P2.6 Stale-Pending Cleanup Race Pattern

- **Location:** `src/planner/expander.rs` (multiple sites, e.g., lines 58–72)
- **Root Cause:** Pattern of dropping a lock, then re-acquiring a different lock, then re-acquiring the first lock to clean up stale entries.
- **Note:** Safe in practice because the planner is single-threaded per job, but fragile.
- **Fix Guidance:** Refactor into a single `clean_stale_pending(...)` helper or document why the pattern is safe.
- **Subagent Scope:** `planner/expander.rs`.

---

## P3 — Low: Cosmetic / Decorative

### P3.1 `ACTIVE_SEARCH_THREADS` Write-Only Counter

- **Location:** `src/scheduler.rs:19`
- **Impact:** Purely decorative metric in `get_pool_status()`. Not harmful.
- **Fix Guidance:** Optional — remove if simplifying.

### P3.2 `#[allow(unused_assignments)]`

- **Location:** `src/planner/engine.rs:624`
- **Root Cause:** Suppresses warning for commented-out `branch.cost += res.cost` at line 703.
- **Fix Guidance:** Remove the comment and the attribute.

---

## Test Coverage Gaps

The Rust integration tests (`tests/planner_integration.rs`) cover basic single-action, empty-plan, no-action, and max-depth cases. They do **not** cover:

1. **Requirement/Provision chaining** (e.g., Pickup → Eat with `held_item` binding).
2. **Wildcard fact provisions** (`FactWildcard` — core to GoToAction).
3. **Binding injection** during forward validation (chain-position semantics).
4. **Custom precondition callbacks** returning `false` or pending.
5. **BestCost vs FirstComplete** behavior with multiple valid plans.
6. **Cancellation** mid-search (`cancel_flag`).
7. **Budget/depth exhaustion** yielding `PlannerRunResult::Pending`.
8. **`BindingInSet` with world group context**.

**Recommendation:** Add a new `tests/requirement_provision_chaining.rs` module covering items 1, 2, 3, and 8.

---

## Subagent Delegation Packages

### Package A — Critical Logic Fixes (P0)
**Owner:** Single subagent (needs deep understanding of search loop).
**Files:** `planner/engine.rs`, `planner/simulation.rs`, `requirement.rs`
**Deliverables:**
1. Delete or correct the post-action precondition check (P0.1).
2. Pass world context into `provision_satisfies_requirement` during forward validation (P0.2).
3. Add regression Rust tests proving both bugs and their fixes.
4. `make test-rust` must pass.

### Package B — Dead-Code & Doc Cleanup (P1) ✅ COMPLETE
**Owner:** Single subagent (mechanical cleanup, low risk).
**Files:** `lib.rs`, `planner/mod.rs`, `planner/engine.rs`, `plan_types.rs`, `plan_tree.rs`, `planner/types.rs`, `debug_tree.rs`, `requirement.rs`
**Deliverables:**
1. ✅ Fixed algorithm characterization in docs (P1.1).
2. ✅ Removed all dead types listed in P1.2 (`RipplePolicy`, `PlanFingerprint`, `RequestKind`, `SearchTree`, `BranchState::Initializing`).
3. ✅ Replaced manual `to_string()` with `Display` impls (P1.3).
4. ✅ `cargo check` and `make test` pass.

### Package C — Scheduler & Expander Refactoring (P2) ✅ COMPLETE
**Owner:** Single subagent.
**Files:** `scheduler.rs`, `planner/types.rs`, `planner/engine.rs`
**Deliverables:**
1. ✅ Extracted `inject_bindings_into_agent` helper (P2.1) — replaces three identical binding-injection blocks.
2. ✅ Extracted generic `extract_typed_specs<T, F>` helper (P2.2) — collapses three near-identical dictionary-extraction functions into one.
3. ✅ Added `PlanBranch::shift_positions(delta: usize)` (P2.3) — replaces three separate index-shifting loops on action prepend.
4. ✅ Extracted `remove_indices<T>(vec, indices)` helper (P2.4) — replaces two manual while-loop removal patterns with a cleaner descending-sort approach.
5. ✅ `cargo check`, `cargo test`, and `make test` all pass.

### Package D — Rust Integration Test Expansion
**Owner:** Single subagent.
**Files:** `tests/` directory
**Deliverables:**
1. New `tests/requirement_provision_chaining.rs` covering items 1–4 and 8 from the Test Coverage Gaps section.
2. New `tests/cancellation.rs` or extend existing with cancel-flag test.
3. New `tests/best_cost_termination.rs` with two valid plans of different costs.
4. `make test-rust` must pass.

---

## Acceptance Criteria for All Packages

- `make test-rust` passes.
- `cargo clippy` emits no new warnings.
- No behavioral regressions in existing Godot integration tests (`make test-godot` if runnable).
- Each package should update this document with a checked-off status.

---

## Cross-Reference: Related Architecture Notes

- `notes/BACKWARD_CHAINING_GOAP_PLANNER_PLAN.md` — Original design intent (backward chaining, not forward search).
- `notes/bugs/CALLBACK_RESUME_DEADLOCK_ANALYSIS.md` — Context on async callback state machine.
- `notes/completed/ASYNC_PLANNER_MIGRATION_PLAN.md` — Context on async-only migration.
