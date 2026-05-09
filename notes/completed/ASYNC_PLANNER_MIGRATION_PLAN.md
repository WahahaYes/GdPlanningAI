# Async Planner Migration Plan

## Goal

Make the background planner the only planner implementation used by the framework, and remove the synchronous planner path once equivalent behavior, API coverage, and test coverage exist.

This note documents what needs to happen to complete that migration safely.

---

## Current State

**Migration complete.** The framework uses a single async planner path.

- `GdPAIPlanScheduler.submit_plan(...)` is the only planning entry point
  - implemented in `addons/GdPlanningAI/rust/src/scheduler.rs` and `addons/GdPlanningAI/rust/src/planner.rs`
- `RustPlanningEngine` and `planning_engine.rs` have been removed
- `GdPAIAgent.manually_start_plan()` uses the async scheduler path
- Missing scheduler is an explicit error, not a silent sync fallback
- `GdPAIRustBridge` is serialization-only; `build_plan()` has been removed
- Log level is configured through `GdPAIPlanScheduler.set_log_level()`
- `max_recursion` is set on the `GdPAIPlanScheduler` instance, not the bridge

---

## Target End State

After migration:

- all framework-driven planning uses the async scheduler path
- there is no runtime fallback from async planning to sync planning
- manual planning also goes through the scheduler
- tests validate the async planner instead of the sync planner
- `planning_engine.rs` and the sync-only planning data path can be removed, unless retained temporarily as an internal compatibility shim

---

## Migration Principles

- keep one authoritative planner behavior
- do not remove sync code until async behavior is functionally equivalent for supported scenarios
- update tests before or alongside removals
- preserve the public user-facing planning workflow where possible
- make any changed behavior explicit in docs and release notes

---

## Required Work

## 1. Decide the public API shape for manual planning

`GdPAIAgent.manually_start_plan()` currently behaves synchronously.

We need to choose one of these:

- make `manually_start_plan()` fire an async request and return immediately
- add a new completion signal/callback contract for manual planning callers
- keep the method name but document that it schedules planning rather than producing an immediate plan

This is the first required decision, because it affects user-facing semantics.

### Recommended direction

Use the scheduler for `manually_start_plan()` as well, and document that it requests a plan rather than completing one immediately.

If consumers need explicit completion hooks, add a signal or documented callback path rather than preserving a hidden sync implementation.

---

## 2. Remove sync entry from `GdPAIAgent`

Update `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd` so that:

- `manually_start_plan()` uses `_start_plan_async()`
- `_start_plan()` is removed, or reduced to a temporary shim during migration
- `_start_plan_async()` no longer falls back to `_start_plan()` when the scheduler is missing
- missing scheduler becomes an explicit configuration/runtime error instead of silently using a different planner

### Why this matters

As long as the fallback exists, the framework still has two planner behaviors in production.

---

## 3. Replace bridge usage that depends on sync planning

The current bridge exposes sync planning through:

- `GdPAIRustBridge.build_plan(...)`

The async path currently uses the bridge only for serialization:

- `serialize_actions(...)`
- `serialize_goals(...)`

We should move to a state where the bridge is serialization-only for planning submission, not a place that can still invoke the sync planner.

### Required changes

- remove or deprecate `build_plan(...)` from `gdpai_rust_bridge.gd`
- keep serialization helpers needed by the scheduler path
- update any direct callers of `build_plan(...)` to use the scheduler flow instead

---

## 4. Replace sync-focused tests with async-focused tests

Current planner tests mainly target `RustPlanningEngine.build_plan(...)` directly.

These need to be replaced or supplemented with tests that validate:

- `GdPAIPlanScheduler.submit_plan(...)`
- callback processing via `process_callbacks()`
- `_on_plan_ready(result)` delivery
- agent plan generation and stale-result handling
- parity for core planning scenarios currently covered by sync tests

### Minimum async test coverage

- empty plan returns failure
- already-satisfied goal returns success with zero cost
- one-action plan succeeds
- lower-cost branch is chosen over higher-cost branch
- cheaper deeper chain beats expensive direct completion
- action validity failures are respected
- action preconditions are respected
- custom preconditions still work
- requirements/provisions flow works under async planning
- stale async results are discarded correctly

### Important note

Before deleting sync tests, confirm the async planner matches intended behavior for all existing examples.

---

## 5. Close known behavior drift before removal

The async planner must be audited for equivalence with the sync planner before the sync implementation is removed.

At minimum verify:

- action validity checks
- action precondition checks
- goal satisfaction checks
- progress-toward-goal logic
- cost evaluation behavior
- requirement/provision propagation
- plan pruning behavior
- dependency validity checks for freed objects

### Critical risk

If the async planner differs semantically from the sync planner, removing the sync implementation without parity tests will silently change planner behavior across the framework.

---

## 6. Remove sync Rust planning path

Once the async path is authoritative and covered by tests, remove sync-specific planning code.

### Candidate removals

- `addons/GdPlanningAI/rust/src/planning_engine.rs`
- sync planner registration from `lib.rs`
- sync-only planner usage in `gdpai_rust_bridge.gd`
- any sync-only Rust data structures that are no longer referenced

### Candidate Rust types to revisit

These may become removable or should be merged into async-only equivalents:

- `ActionData`
- `GoalData`
- `PreconditionHandler.evaluate(...)` as a planner-time sync path
- any sync-only deserialization helpers used only by `RustPlanningEngine`

This step should happen only after the async tests are in place.

---

## 7. Remove `RustPlanningEngine` directly once migration is complete

No compatibility shim is needed here. This is not a deployed app with a stable external runtime contract; it is a game-loop framework under active development.

Once the framework no longer calls into the sync planner and the async path has equivalent test coverage, `RustPlanningEngine` should be removed rather than preserved as a wrapper or deprecated shell.

### Removal condition

- all internal callsites have been moved to the scheduler path
- async planner coverage replaces sync planner coverage
- docs and examples no longer instruct users to instantiate `RustPlanningEngine`

### Recommended direction

Remove `RustPlanningEngine` entirely once the async migration is complete.

---

## 8. Update documentation and examples

Documentation should reflect async-only planning behavior.

### Update locations

- `README.md`
- any scheduler/threading notes
- architecture notes that still describe sync planner usage
- examples or tutorials that instantiate `RustPlanningEngine` directly
- manual planning docs if completion semantics change

---

## Suggested Execution Order

## Phase 1: Lock behavior and tests — COMPLETE

- audit async planner for sync parity
- add async integration tests for all existing sync planner scenarios
- confirm hunger and other example scenes behave correctly under async planning only

## Phase 2: Flip GDScript runtime to async-only — COMPLETE

- make `manually_start_plan()` async
- remove sync fallback from `_start_plan_async()`
- stop using `_bridge.build_plan(...)`

## Phase 3: Remove sync Rust path — COMPLETE

- remove `planning_engine.rs`
- remove sync bridge methods and dead data structures
- update `lib.rs`
- clean up any docs or tests that still mention sync planning

## Phase 4: Cleanup and release notes — COMPLETE

- document any public API changes
- document manual planning completion semantics
- document scheduler requirement explicitly

---

## Acceptance Criteria

The migration is complete when all of the following are true:

- no framework runtime path calls `RustPlanningEngine.build_plan(...)`
- `GdPAIAgent.manually_start_plan()` no longer uses sync planning
- missing scheduler does not silently switch planners
- test coverage exists for async planning behavior and async result delivery
- planner examples run correctly using async-only planning
- sync planner Rust files and sync-only bridge methods are removed or intentionally deprecated

---

## Open Questions

- should `manually_start_plan()` keep its current name if it becomes asynchronous?
- do we want a signal for plan completion in addition to `_on_plan_ready(result)`?
- do we need a temporary compatibility wrapper for external users of `RustPlanningEngine`?
- should the planner core be shared/refactored before removing the sync path, or should we first complete the async-only migration and then consolidate internals?

---

## Recommendation

Complete the migration in two passes:

- first make async planning the only runtime path and replace test coverage
- then remove the sync planner implementation and dead bridge code

That sequence minimizes risk and makes behavior changes visible before architecture cleanup deletes the old path.
