# Bug: Inaccurate GoTo Cost in Backward Search

## Problem
The planner's backward search was unable to distinguish between multiple `GoToAction` candidates because their costs were being calculated as a fixed heuristic (e.g., `10.0`) during the search phase. This occurred because:
1. The `at_target` binding from the planner's candidate search was not being resolved to a spatial location during `get_action_cost`.
2. Action instances were shared across the plan, but their state (like `target_location`) was only injected *after* planning, making them effectively stateless during the actual search.

This caused the planner to prune optimal branches, as all navigation actions appeared equally expensive regardless of actual world distance.

## Fix
1. **Enhanced Blackboard Lookups**: Updated `GdPAIBlackboard.get_object_for` in Rust to handle raw instance IDs and UIDs, allowing GDScript cost functions to resolve `SimObjectProxy` snapshots from planner-provided IDs.
2. **Context-Aware Costing**: Updated `GoToAction.get_action_cost` and `simulate_effect` to prefer bindings stored in the simulated blackboard (`at_target`) over the `target_location` member variable.
3. **Execution Safety**: Confirmed that `GdPAIAgent` correctly re-injects bindings into shared action instances before every `perform_action` call, and that `set_state`/`get_state` uses `chain_position` to isolate persistent state between repeated actions in a single plan.

## Impact
The planner now correctly calculates Euclidean distance for every `Go To` candidate during backward search, ensuring it always selects the nearest valid target. End-users are not required to implement cloning logic, keeping the API simple.
