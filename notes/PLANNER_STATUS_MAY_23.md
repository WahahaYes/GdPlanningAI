# Planner Status - May 23, 2026

## Summary
The core planning engine in Rust has been fully refactored to an async-first state machine. All synchronous entry points have been removed from production. The engine now supports cost-optimal search using actual simulation data provided by GDScript via non-blocking callbacks.

## Current Progress
- [x] **Async-First Core**: `PlannerEngine` is now a state machine that yields `Pending` when it needs Godot data.
- [x] **Dijkstra Implementation**: Switched from A* to Dijkstra (zero heuristic). The previous heuristic (need count) was non-admissible because one action satisfies multiple needs.
- [x] **Cost Source of Truth**: Added `action_costs` cache and `recalculate_cost()` to `PlanBranch`. This ensures costs from async callbacks aren't lost between simulation steps and prevents doubling errors.
- [x] **Discovery Loop Fix**: Fixed a "busy-wait" bug where `find_candidates` failed to park nodes with a valid `request_id` for already-pending discoveries.
- [x] **Frontier Logic Restoration**: Restored the logic that removes satisfied needs from the `open_needs` list when prepending an action.
- [x] **Parallel Discovery**: `find_candidates` triggers multiple simulation requests in parallel to avoid sequential round-trip bottlenecks.
- [x] **Budget Management**: The `GdPAIPlanScheduler` correctly handles budget-exhausted vs callback-waiting yields.
- [x] **Rust Test Parity**: All Rust integration tests (`tests/planner_integration.rs`) and library unit tests are passing.

## Outstanding Issues
1. **Godot Integration Failures**: Complex scenarios in `test_campfire_example_smoke.gd` and `test_requirements_provisions.gd` are still reporting failures.
   - **Cost Mismatches**: Some tests report total cost as 2.0 when 1.0 is expected, or 20.0 when 2.0 is expected.
   - **Empty Plans**: The "full cooking chain" returns empty plans in some scenarios.
2. **Infinite Simulation Loop Risk**: `process_simulation` was simplified to handle one step at a time to ensure callback responses are correctly cleared and not re-requested.

## Implementation Decisions vs. Pseudocode
The following implementation details were added or modified compared to the original `notes/PLANNER_ALGORITHM_PSEUDOCODE.md`:

1. **Dijkstra for Optimality**: Switched to $h=0$ because our hybrid symbolic/simulation model makes creating an admissible heuristic difficult.
2. **Explicit Cost Caching**: `PlanBranch` now uses `action_costs: Vec<f64>` as the source of truth for cost summation.
3. **Yield on Every Step**: `process_simulation` now yields `Ready(())` after satisfying a single precondition or simulating one action forward. This ensures the engine re-queues and checks the priority queue frequently, which is safer for async and ensures we always pick the cheapest path even if it's currently "rippling".
4. **Frontier Update**: During expansion, the specific indices of satisfied needs are tracked and removed from the branch's open needs list.
5. **Partial Dictionary Guard**: Updated `scheduler.rs` to return a full `success=false` PlanResult dictionary when search is exhausted, preventing GDScript "invalid key" errors.
