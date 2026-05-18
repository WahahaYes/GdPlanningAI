# Campfire and Hunger Test Failures Investigation

## Current Status
- **Passing Tests**: 43/46
- **Failing Tests**:
  - `res://test/integration/test_campfire_example_smoke.gd`: `test_full_cooking_chain`, `test_preemptive_fire_maintenance`
  - `res://test/integration/test_hunger_example_smoke.gd`: `test_real_hunger_example_shakes_tree_then_picks_up_food`

## Accomplishments & Progress (May 18, 2026)

### 1. Fixed Goal Lambda Stability
- **Problem**: Goal lambdas with typed array annotations (`Array[Precondition]`) were causing silent runtime failures in the background planning thread pool.
- **Solution**: Removed typed array annotations from goal lambdas. This cleared silent runtime errors in background callbacks.

### 2. Verified World State Visibility
- **Problem**: Uncertainty if the agent "sees" objects added dynamically (e.g., dropped fruit).
- **Solution**: Increased `_pump_frames` to 10 in test setup. Verified via assertions that `_collect_worldly_actions()` correctly registers new objects and their provided actions.

### 3. Action-Led Hypothetical Progress
- **Problem**: `AddFuelAction` wasn't being discovered as a candidate because its fuel-increasing effect was only reported if the agent already held wood.
- **Solution**: Updated `AddFuelAction.simulate_effect` to report its effects even if the `held_item` requirement isn't physically met yet. This allows the planner's discovery phase to see it as a candidate.

### 4. Stabilized Callables
- **Problem**: "Target object freed" errors when evaluating custom preconditions.
- **Solution**: Replaced `custom_with_deps` in `PickupAction` and `ShakeTreeAction` with standard `custom` checks and internal `is_instance_valid` guards.

### 5. Enhanced Planner Observability
- **Action**: Added detailed `log_info` and `log_debug` instrumentation to Rust `expander.rs`, `engine.rs`, and `scheduler.rs`.
- **Finding**: Confirmed that `AddFuelAction` **is** being discovered as a candidate for the `Maintain Fire` goal, but the A* search is failing to ground or complete the branch.

## Key Learnings & Remaining Blockers

### The "Grounding" Problem
- **Hunger Test**: The agent chooses to `Shake Tree` again instead of picking up existing food. This suggests a failure in the planner's ability to ground the `Pickup` action against existing world objects or a cost/heuristic bias.
- **Campfire Test**: The `Maintain Fire` goal (custom lambda) fails to return `true` even in simulated states where fuel was increased. This points to potential issues in how object group memberships are preserved in snapshots.

## Next Steps
1. **Audit `expander.rs` Ripple Phase**: Verify `apply_requirements_to_snapshots` preserves object group memberships.
2. **Debug Snapshot Group Lookup**: Investigate if `SimObjectProxy::is_in_group` in Rust correctly reflects groups of dynamic objects.
3. **Review Heuristic Weights**: Ensure the heuristic isn't excessively pruning branches leading to grounded world objects.
