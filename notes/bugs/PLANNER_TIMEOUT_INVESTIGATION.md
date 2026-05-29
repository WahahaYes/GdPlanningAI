# Bug: Planner Search Stalls & Thread Leakage

**Status**: Significant Breakthroughs / Investigating Pruning
**Severity**: High (Causes CI timeouts and inconsistent test results)

## Symptoms
- Integration tests (specifically campfire scenarios) time out after 2-5 seconds.
- `POOL STATUS` revealed `Active Jobs` count increasing monotonically (Now Fixed).
- Search terminates with `FAILURE — no plan found` despite valid-looking partial trees.

## Fixed / Resolved

### 1. Resurrection of Cancelled Jobs
Added `cancel_flag` check in `scheduler.rs` before re-spawning yielded engines. Thread pool saturation is resolved.

### 2. Async Callback Reaping
Implemented deferred reaping (`pending_reap` flag) in `scheduler.rs`. GDScript now consistently captures debug trees before jobs are purged.

### 3. Premature Simulation Pruning (The "Rippling" Deadlock)
Refactored `engine.rs` state machine to stay in `Searching` state until all symbolic requirements are met. This allows the planner to find providers (like `Pick Up Wood`) before trying to simulate actions that depend on them (like `Add Fuel`).

### 4. Causal Link Congestion (Requirement Duplication)
Implemented **Greedy Requirement Clearing** and **Split Constraints** in `expander.rs`.
- **Requirements**: Strictly sequential (`pos 0`).
- **Preconditions**: Unrestricted (state persistence).
- **Result**: Branches no longer accumulate redundant `at_target` or `exists` needs. A single `Go To` now satisfies all identical requirements in the chain.

### 5. Scavenger Loops (Wood Eating)
Refined `EatHeldFoodAction.gd` to require the `is_food` fact.
- **Result**: The planner no longer tries to eat wood to clear its hands for picking up more wood. Causal loops between non-food items and eating are broken.

## Current Root Causes Under Investigation

### 1. State Oscillation (PickUp -> Drop Loop)
The planner is caught in a new loop: `PickUp Wood` -> `Drop Item` -> `PickUp Wood`.
- **Cause**: `PickUp` needs empty hands; `Drop` provides empty hands. `Drop` needs an item; `PickUp` provides an item.
- **Problem**: The `visited` cache allows this because each cycle adds cost, and `cost >= prev_cost` only prunes if the cost stays the same or decreases.
- **Evidence**: Debug trees show infinite back-and-forth between these two actions.

## Remaining Work
- [ ] **Ancestry Loop Detection**: Modify `engine.rs` to prune a branch if it repeats a logical state (same open needs) that already exists in its own ancestry, regardless of cost.
- [ ] **Pseudocode Alignment**: Update the project's planning documentation to reflect the new hybrid State Machine (Searching -> Verifying) and the Split Constraint discovery model.
- [ ] **Optimization**: Reduce redundant discovery simulations during node resumption.
