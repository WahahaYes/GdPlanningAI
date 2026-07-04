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

### P0.1 Post-Action State Checked Against Action's Own Preconditions ✅ FIXED

- **Location:** `src/planner/engine.rs`, lines 508–526
- **Root Cause:** After discovery-simulating a candidate action, the code removed preconditions at `pos == 0` if they happened to be satisfied by the **post-action** discovery state. Preconditions must hold *before* an action runs, not after.
- **Fix Applied (2026-07-03):** Deleted the entire block. The `satisfied_by_initial` check at lines 441–450 is the correct and sufficient logic — preconditions already met by the initial state are never pushed, and any that remain must be satisfied by a predecessor.
- **Regression Test:** `tests/requirement_provision_chaining.rs::action_cannot_satisfy_its_own_precondition` proves that an action whose effect satisfies its own precondition is still rejected when the initial state does not satisfy it.

### P0.2 `BindingInSet` Requirements Weakened During Forward Validation ✅ FIXED

- **Location:** `src/planner/engine.rs`, lines 706–712
- **Root Cause:** `process_simulation` called `provision_satisfies_requirement(prov, req, None)` with `world = None`. In `src/requirement.rs:317–336`, `BindingInSet` fell back to a simple name match when `world` was `None`, ignoring the group-membership constraint.
- **Fix Applied (2026-07-03):** Replaced `None` with `Some(&branch.current_world)` so the world context is available for group-membership validation.
- **Regression Test:** `tests/requirement_provision_chaining.rs::binding_in_set_rejected_during_forward_validation` proves that a non-food item (sword) is correctly rejected as satisfying a `BindingInSet { held_item, "food" }` requirement.

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

### P2.5 Cost-Caching via Mutable Slice Side Channel ✅ FIXED

- **Location:** `src/planner/simulation.rs`, lines 82–91
- **Root Cause:** `simulate_action` mutates `branch_action_costs[simulation_index]` through a mutable slice in `SimArgs`, creating a hidden caching channel.
- **Fix Applied:** Added explicit doc comments to `SimArgs` and `simulate_action` explaining that `branch_action_costs` serves as a per-branch action cost cache, and that the function reads from and writes back to `branch_action_costs[simulation_index]`. Callers must pass the same mutable slice on resumption so previously fetched costs are reused.
- **Subagent Scope:** `planner/simulation.rs`.

### P2.6 Stale-Pending Cleanup Race Pattern ✅ FIXED

- **Location:** `src/planner/expander.rs` (multiple sites)
- **Root Cause:** Pattern of dropping a lock, then re-acquiring a different lock, then re-acquiring the first lock to clean up stale entries. Also contained a latent variable-shadowing bug (`let mut pending = ctx.discovery_request_map.lock()` assigned to `pending`).
- **Fix Applied:** Extracted `check_pending_or_clean_stale<K>(pending_map, request_map, key) -> Option<usize>` helper in `expander.rs`. Replaced all 4 occurrences of the stale-entry cleanup pattern with calls to this helper. The helper correctly acquires the pending map, checks the request map, and removes stale entries atomically.
- **Subagent Scope:** `planner/expander.rs`.

---

## P3 — Low: Cosmetic / Decorative

### P3.1 `ACTIVE_SEARCH_THREADS` Write-Only Counter ✅ FIXED

- **Location:** `src/scheduler.rs:19`
- **Impact:** Purely decorative metric in `get_pool_status()`. Not harmful.
- **Fix Applied:** Deleted the `ACTIVE_SEARCH_THREADS` static, removed `fetch_add`/`fetch_sub` calls in `run_job_step`, and removed the field from the `get_pool_status()` format string.

### P3.2 Stale Commented-Out Code ✅ FIXED

- **Location:** `src/planner/engine.rs:682`
- **Root Cause:** Misleading commented-out `branch.cost += res.cost` implying a design question that was resolved in Package C (`recalculate_cost` is the source of truth).
- **Fix Applied:** Deleted the commented-out line.

---

## Test Coverage Gaps ✅ CLOSED

The Rust integration tests (`tests/planner_integration.rs`) cover basic single-action, empty-plan, no-action, and max-depth cases. The gaps below were closed by Package D:

1. ✅ **Requirement/Provision chaining** — `tests/requirement_provision_chaining.rs::requirement_provision_chains_pickup_then_eat`
2. ✅ **Wildcard fact provisions** (`FactWildcard`) — `tests/requirement_provision_chaining.rs::wildcard_fact_provision_binds_concrete_value`
3. ✅ **Binding injection** during forward validation — `tests/requirement_provision_chaining.rs::binding_injection_available_during_forward_validation`
4. ✅ **Custom precondition callbacks** returning `false` — `tests/requirement_provision_chaining.rs::custom_precondition_false_prunes_branch`
5. ✅ **BestCost vs FirstComplete** — `tests/best_cost_termination.rs::best_cost_returns_lowest_cost_plan` + `first_complete_returns_first_found_plan`
6. ✅ **Cancellation** mid-search — `tests/cancellation.rs::cancellation_returns_failure_quickly`
7. ✅ **Budget/depth exhaustion** yielding `PlannerRunResult::Pending` — `tests/budget_exhaustion.rs::iteration_budget_exhaustion_returns_pending` and `max_depth_prevents_plan_completion`.
8. ✅ **`BindingInSet` with world group context** — `tests/requirement_provision_chaining.rs::binding_in_set_respects_world_group`

---

## Subagent Delegation Packages

### Package E — Critical Logic Fixes (P0) ✅ COMPLETE
**Owner:** Single subagent (needs deep understanding of search loop).
**Files:** `planner/engine.rs`, `tests/requirement_provision_chaining.rs`
**Deliverables:**
1. ✅ Deleted the post-action precondition check block (P0.1).
2. ✅ Replaced `None` with `Some(&branch.current_world)` in forward validation `provision_satisfies_requirement` call (P0.2).
3. ✅ Added regression Rust tests `action_cannot_satisfy_its_own_precondition` and `binding_in_set_rejected_during_forward_validation`.
4. ✅ `make test-rust` passes.

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

### Package D — Rust Integration Test Expansion ✅ COMPLETE
**Owner:** Single subagent.
**Files:** `tests/` directory
**Deliverables:**
1. ✅ `tests/requirement_provision_chaining.rs` covering items 1–4 and 8 from the Test Coverage Gaps section (5 tests).
2. ✅ `tests/cancellation.rs` with cancel-flag mid-search test.
3. ✅ `tests/best_cost_termination.rs` with two valid plans of different costs (2 tests).
4. ✅ `make test-rust` passes — total Rust test count increased from 31 to 44 tests.

### Package F — Remaining Audit Cleanup (P2.5, P2.6, P3.1, P3.2, Test Gap #7) ✅ COMPLETE
**Owner:** Single subagent.
**Files:** `planner/simulation.rs`, `planner/expander.rs`, `planner/engine.rs`, `scheduler.rs`, `tests/budget_exhaustion.rs`
**Deliverables:**
1. ✅ P2.5 — Documented cost-caching side channel in `SimArgs` and `simulate_action` docs.
2. ✅ P2.6 — Extracted `check_pending_or_clean_stale` helper; fixed latent variable-shadowing bug in stale-pending cleanup.
3. ✅ P3.1 — Removed `ACTIVE_SEARCH_THREADS` static and its usage.
4. ✅ P3.2 — Deleted stale commented-out code in `engine.rs`.
5. ✅ Test Gap #7 — Added `tests/budget_exhaustion.rs` with `iteration_budget_exhaustion_returns_pending` and `max_depth_prevents_plan_completion`.
6. ✅ `cargo check`, `cargo test`, `cargo clippy` pass (no new warnings).

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
