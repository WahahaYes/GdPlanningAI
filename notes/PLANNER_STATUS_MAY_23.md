# Planner Status - May 23, 2026

## Summary
The core planning engine in Rust has been fully refactored to an async-first state machine. All synchronous entry points have been removed from production. The engine now supports A* search using actual simulation data (cost, effects, preconditions) provided by GDScript via non-blocking callbacks.

## Current Progress
- [x] **Async-First Core**: `PlannerEngine` is now a state machine that yields `Pending` when it needs Godot data.
- [x] **A* Implementation**: Replaced `VecDeque` with `BinaryHeap` and implemented a heuristic-aware `priority()` for `SearchNode`.
- [x] **Cost Optimality**: Refactored the `visited` cache from a `HashSet` to a `HashMap<State, Cost>` to allow A* to find cheaper paths to the same state.
- [x] **Parallel Discovery**: `find_candidates` now triggers multiple simulation requests in parallel to avoid sequential round-trip bottlenecks.
- [x] **Budget Management**: The `GdPAIPlanScheduler` now correctly distinguishes between "yield for budget" (resumes next frame) and "yield for callback" (resumes when data arrives).
- [x] **Rust Test Parity**: All Rust integration tests (`tests/planner_integration.rs`) and library unit tests are passing.

## Outstanding Issues
1. **Godot Integration Failures**: Several complex scenarios in `test_campfire_example_smoke.gd` are failing.
   - **Incorrect Cost Selection**: The planner sometimes chooses a path with cost 10.0 when a 1.0 path exists.
   - **Empty Plans**: Scenarios like the "full cooking chain" return empty plans despite a valid chain existing.
   - **Timeouts**: Some tests hit the 5-second timeout waiting for a result, suggesting either an infinite loop in discovery or search exhaustion.
2. **Infinite Simulation Loop Risk**: If a simulation step (e.g. `simulate_action`) returns `Pending` but the engine doesn't correctly clear/consume the response, it can re-request the same data indefinitely.
3. **Verification Logic**: Ensure that the `Verifying` state (forward validation of the entire chain) correctly prunes based on the **total accumulated cost** found during simulation vs. the symbolic estimate.

## implementation Decisions vs. Pseudocode
The following implementation details were added or modified compared to the original `notes/PLANNER_ALGORITHM_PSEUDOCODE.md`:

1. **Explicit Cost Caching**: `PlanBranch` now has an `action_costs: Vec<f64>` field. This was necessary because a single action simulation in the async model involves two distinct requests (Cost then Effect). Without this cache, the cost obtained in step 1 was lost when the branch yielded to wait for step 2.
2. **Parallel Discovery**: The pseudocode implied a sequential "discovery" per action. Implementation optimized this by scanning all actions and firing off all missing discovery requests in one pass.
3. **Budget-Aware Yielding**: Added `pending_request_id` to the scheduler. `id == 0` means "yielded for budget", `id > 0` means "yielded for callback". This prevents budget-exhausted jobs from stalling while waiting for a response that will never come.
4. **Visited-Cost Pruning**: The implementation uses a `HashMap` for visited states to support A*'s requirement that we only discard a state if we've reached it with a *lower* cost previously.
5. **Clear Callback Responses**: Modified `process_simulation` to clear the `callback_response` only after it has been fully consumed by the state machine (e.g., after the effect is applied, not just after the cost is read).
