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

- [x] **Serial Goal Processing**: Implemented priority-based serial goal execution. The planner now searches for the highest-reward goal first and only moves to the next goal if the first one is impossible. This fixed a major bug where the "Maintain Fire" goal (already satisfied, 0 cost) was winning over the "Hunger" goal (needs actions, >0 cost).
- [x] **Serialization Verification**: Confirmed that bit-pattern logs (e.g. `Float(463...)`) are correct `f64` representations and are being interpreted correctly by Rust. The "corrupted float" theory was debunked.

## Outstanding Issues
1. **Godot Integration Failures**: Complex scenarios in `test_campfire_example_smoke.gd` and `test_requirements_provisions.gd` are still reporting failures.
   - **Plan Short-Circuiting**: In the "full cooking chain" test, the planner correctly picks `Go To -> Dig Potato -> Eat` because it is cheaper than the cooking chain and the test's hunger threshold allows raw food to satisfy the goal.
   - **Cost Selection**: Some tests still report mismatched total costs (e.g. 10.0 vs 2.0), likely due to how action costs are aggregated in complex chains.

## Implementation Decisions vs. Pseudocode
1. **Reward-Sorted Serial Search**: Modified the engine to be goal-serial. This ensures agents always pursue their most valuable unsatisfied goal first, matching the "Utility GOAP" mental model while keeping the performance benefits of Dijkstra on a per-goal basis.
2. **Yield on Goal Transition**: When search for goal N is exhausted, the engine yields a `Pending(0)` before starting goal N+1. This prevents a single frame from being locked up by searching through an entire stack of impossible goals.
