# Bug: Planner Search Stalls & Thread Leakage

**Status**: Investigating / Significant Progress
**Severity**: High (Causes CI timeouts and inconsistent test results)

## Symptoms
- Integration tests (specifically campfire scenarios) time out after 2-5 seconds.
- Debug trees previously showed very low branch counts (1-20) despite long execution times.
- `POOL STATUS` revealed `Active Jobs` count increasing monotonically across test runs.
- Search often terminated with `FAILURE — no plan found` even when valid chains exist.

## Fixed / Resolved

### 1. Resurrection of Cancelled Jobs
In `scheduler.rs`, jobs that yielded for budget (`iterations > 20000`) were being re-spawned into the Rayon pool without checking if they had been cancelled while yielded.
- **Fix**: Added `cancel_flag` check in `process_callbacks` before `run_job_step`.
- **Result**: Thread pool saturation from "zombie" search threads is resolved.

### 2. Async Callback Reaping
If a test timed out, the Rust engine remained "parked" waiting for callbacks that would never come.
- **Fix**: Added `cancel_all_jobs()` to test `after_each`.
- **Improvement**: Implemented a deferred reaping logic in `scheduler.rs` (`pending_reap` flag) that gives GDScript one frame to extract the final debug tree before the job is purged.

### 3. Premature Simulation Pruning (The "Rippling" Deadlock)
The planner was attempting to forward-simulate (`Rippling` state) action effects immediately after prepending an action, even if its symbolic requirements (like `held_item`) weren't met yet.
- **Cause**: Godot simulation would return "no effect" (e.g., cannot eat if not holding food), leading the planner to believe the branch could not satisfy the goal and pruning it.
- **Fix**: Refactored the state machine in `engine.rs` to stay in `Searching` state as long as symbolic requirements or preconditions are open.
- **Result**: Planner can now successfully chain deep plans (reaching depth 4 in campfire tests) by finding all causal dependencies before validating the grounded state.

## Current Root Causes Under Investigation

### 1. Causal Link Congestion (Requirement Duplication)
When multiple actions in a chain introduce specific state requirements (e.g., `at_target(PotatoPatch)` followed by `at_target(Campfire)`), the planner accumulates them in `open_requirements`.
- **Problem**: The discovery logic tries to satisfy *all* open requirements. If a branch has two `at_target` needs, it may prepend two `Go To` actions in a row, which results in a simulation failure during verification.
- **Evidence**: Debug trees show branches with `open_req=[at_target(...), at_target(...), exists(held_item)]`.

### 2. Hybrid Simulation Latency
Every branch expansion that requires a Godot callback adds ~16.6ms of latency.
- **Progress**: Refactored `find_candidates` to return `Ready` candidates immediately. However, the ROOT node still spends significant time "parked" waiting for the initial discovery simulations of all 15+ available actions.

## Remaining Work
- [ ] **Sequential Requirement Handling**: Modify `engine.rs` to prioritize satisfying the "most recent" requirement (pos 0) and avoid duplicating requirements of the same name for the same state property.
- [ ] **Audit `visited` Cache**: Ensure that bit-display float inconsistencies aren't causing identical states to be treated as unique, or unique states to be pruned as identical.
- [ ] **Optimization**: Reduce redundant `get_discovery_result` calls when a node is resumed after a partial expansion.
