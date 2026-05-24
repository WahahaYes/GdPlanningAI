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

- [x] **Cross-Chain Discovery**: Fixed a major logic error where candidate actions were only allowed to satisfy preconditions at `pos == 0`. They can now satisfy any downstream open precondition in the chain, which is essential for spatial actions like "Go To" that enable later state-changes.

## Outstanding Issues
1. **Godot Integration Failures**: Complex scenarios in `test_campfire_example_smoke.gd` and `test_requirements_provisions.gd` are still reporting failures.
   - **Numeric Comparison Bugs**: Initial state checks for hunger (`70.0 < 30.0`) are returning `true`, leading to "Empty Plan" (already satisfied) results.
   - **Strange Snapshot Bit Patterns**: Logs show `hunger` values like `Float(4632243402438040450)`, suggesting a type or endianness issue in the blackboard serialization.

## Implementation Decisions vs. Pseudocode
The following implementation details were added or modified compared to the original `notes/PLANNER_ALGORITHM_PSEUDOCODE.md`:

1. **Dijkstra for Optimality**: Switched to $h=0$ because our hybrid symbolic/simulation model makes creating an admissible heuristic difficult.
2. **Stable Dijkstra Priority**: Added `symbolic_cost` to `PlanBranch`. Previously, Dijkstra was using the "grounded cost" (re-calculated during simulation), which fluctuates and was causing optimal paths to be incorrectly pruned.
3. **Yield on Every Step**: `process_simulation` now yields `Ready(())` after satisfying a single precondition or simulating one action forward. This ensures the engine re-queues and checks the priority queue frequently, which is safer for async and ensures we always pick the cheapest path even if it's currently "rippling".
4. **Parallel Discovery**: `find_candidates` triggers multiple simulation requests in parallel to avoid sequential round-trip bottlenecks.
5. **Frontier Update**: During expansion, the specific indices of satisfied needs are tracked and removed from the branch's open needs list.
