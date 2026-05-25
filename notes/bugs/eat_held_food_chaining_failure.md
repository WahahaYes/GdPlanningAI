# Bug: Planning Failure during EatHeldFood/Pickup Chaining

## Status
**Date**: May 24, 2026  
**Context**: Integration testing with `test_hunger_example_smoke.gd`.  
**Symptom**: Agent fails to generate a plan when hungry (hunger = 30.0) even after a fruit has been dropped in the world.

## Findings

### 1. Chain Identification
The planner correctly identifies the goal:
- **Goal**: Hunger (Precondition: `hunger < threshold`)
- **Action Chain**: `GoTo` -> `PickupAction` -> `EatHeldFoodAction`

### 2. The Bottleneck: Simulation Grounding
During the expansion and rippling phases, the `EatHeldFoodAction` requires two things:
1. `RequirementSpec.binding_exists("held_item")`
2. `RequirementSpec.fact("is_food", [])`

The `PickupAction` provides these:
- `ProvisionSpec.binding("held_item", item_id)`
- `ProvisionSpec.fact("is_food", [])`

**Observed Issue**: 
The planner successfully finds the candidates, but the `PickupAction` discovery simulation result shows `hunger` being set to `0` or not reflecting the expected state for the next step. 

Debug logs show:
```
Discovery Simulation result for action 1 (EatHeldFood): UpdatedSnapshots(...)
```
However, the `EatHeldFoodAction` simulation relies on the `held_item` property being set in the blackboard to determine how much hunger to restore. If the binding propagation or the order of simulation updates is slightly off during the "Ripple" phase, `EatHeldFoodAction` might be simulating with an empty `held_item` or an incorrect `hunger` value, failing to satisfy the goal's precondition.

### 3. Potential Root Causes
- **Initial Requirement Grounding**: We added logic to clear requirements satisfied by the initial state at `simulation_index == 0`. We need to ensure this isn't clearing requirements that were supposed to be met by the *first action in the chain* (which is at index 0 after prepend).
- **Binding Offset**: We adjusted the consumer binding offset to `consumer_pos` (from `consumer_pos + 1`). If the `action_chain` indices and `action_bindings` indices are misaligned during the ripple, the `EatHeldFoodAction` won't receive the `held_item` ID from the `PickupAction`.
- **Wander Priority**: When hunger is low, `Wander` succeeds. When hunger is high, if the `Eat` chain fails (due to the issues above), the planner returns an empty plan instead of a suboptimal one, or simply times out because it's stuck in a discovery loop.

## Next Steps for Investigation
1. **Trace `action_bindings` during Ripple**: Add logs in `engine.rs` inside `process_simulation` to print `current_bindings` exactly when `simulation_index` matches the `EatHeldFoodAction`.
2. **Verify `PickupAction` Provision**: Ensure `item_id` is correctly captured and passed through the `VariantSnapshot`.
3. **Requirement Satisfaction Check**: Check if `RequirementSpec.binding_exists("held_item")` is being correctly cleared by the `PickupAction`'s provision during the ripple.
4. **Isolate `EatHeldFoodAction`**: Test if the action can plan successfully if the agent *starts* with the food already in its `held_item` slot (bypassing the Pickup step).

## Files Involved
- `@/c:/Godot/GdPlanningAI/addons/GdPlanningAI/rust/src/planner/engine.rs` (Ripple & Binding injection)
- `@/c:/Godot/GdPlanningAI/addons/GdPlanningAI/rust/src/planner/expander.rs` (Candidate discovery)
- `@/c:/Godot/GdPlanningAI/examples/behaviors/campfire/eat_held_food_action.gd` (Simulation logic)
- `@/c:/Godot/GdPlanningAI/examples/objects/holdable/pickup_action.gd` (Provision logic)
