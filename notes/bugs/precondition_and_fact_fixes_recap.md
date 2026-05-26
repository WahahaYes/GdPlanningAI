# Precondition Tracking and Fact Resolution Fixes

The following bugs were identified and fixed in the planner's core engine and example behaviors to ensure reliable 3-depth planning (Go To -> Pickup -> Eat).

## Core Engine Fixes (Rust)

### 1. Missing Blackboard Property Handling
- **Issue**: `GdPAIBlackboard.get_property` returns `null` for missing keys in Godot. The Rust planner was returning `false` for any equality comparison against a missing property.
- **Fix**: Modified `snap_equal` in `plan_types.rs` to treat missing properties as `Nil`.
- **Result**: Preconditions like `held_item == ""` (empty string) now correctly evaluate to `true` when the property hasn't been set yet.

### 2. Over-Aggressive Precondition Satisfaction
- **Issue**: In the backward discovery phase, candidate actions were satisfying preconditions at any index in the chain.
- **Fix**: Restricted candidate actions prepended at `pos=0` to only satisfy preconditions belonging to the immediate next action (original `pos=0`, now `pos=1`).
- **Result**: Prevents the planner from incorrectly "skipping" required intermediate actions.

### 3. Forward Simulation (Rippling) Debug Visibility
- **Change**: Added detailed trace logging to `engine.rs` during the Rippling phase.
- **Benefit**: Clearly shows which preconditions are failing and what the agent's simulated properties look like at each step.

## Example Behavior Fixes (GDScript)

### 4. Fact Chaining Stall in Eat Action
- **Issue**: `EatHeldFoodAction` required a symbolic `is_food` fact. Since facts are transient and only provided by `PickupAction`, the agent could not "prove" that an item already in its hand at the start of a plan was edible.
- **Fix**: Removed the `is_food` fact requirement.
- **Result**: The action now validates edibility via its internal `hunger_restored_by_item` dictionary during simulation and execution, allowing the agent to eat items it was already holding.

### 5. Goal Preference and Action Costing
- **Issue**: `WanderAction` had a high fixed cost (10.0), causing the agent to prefer gathering food (Lower cost ~6.5) even to satisfy the "move distance" requirement of the Wander goal.
- **Fix**: Lowered `WanderAction` cost to 1.0.
- **Result**: The agent now correctly treats aimless wandering as the path of least resistance when not driven by high-priority hunger.
