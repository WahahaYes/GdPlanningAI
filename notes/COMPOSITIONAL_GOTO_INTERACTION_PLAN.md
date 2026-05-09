# Compositional GoTo + Interaction Implementation Plan

## Overview

Replace the bundled `SpatialAction` pattern with a compositional approach where:
- **Agent provides** `GoToAction(target_id)` that handles navigation and provides `at_target(target_id)` provision
- **Objects provide** interaction actions that require `at_target(target_id)` and focus only on domain logic
- The backward-chaining planner naturally chains: Agent GoTo → Object Interaction → Goal

## Key Architectural Change

**Incorrect approach (initial attempt):**
- Objects provide both GoToAction and InteractionAction
- Each object has its own GoToAction instance

**Correct approach:**
- Agent provides a single, parameterized GoToAction
- Objects provide only interaction actions with `at_target` requirements
- The planner chains the agent's GoToAction to satisfy object interaction requirements

## Design Pattern

### Backward Chaining Flow

The planner works backwards from the goal through requirements:

1. **Goal**: reduce hunger
2. **EatHeldFoodAction** satisfies goal, introduces requirement: `binding_exists("held_item")`
3. **PickupInteractionAction** (from FoodObject) provides `held_item`, introduces requirement: `fact("at_target", [food_target_id])`
4. **Agent's GoToAction(food_target_id)** provides `at_target(food_target_id)`

Result: Planner selects `Agent GoTo → PickupInteractionAction → EatHeldFoodAction`

### Key Insight

The agent provides a single parameterized GoToAction that can satisfy any `at_target` requirement. Objects declare their location dependency via `at_target` requirements, and the planner chains the agent's GoToAction to satisfy them.

## Files to Create

### 1. `examples/behaviors/hunger/go_to_action.gd` (Agent-provided)

Generic navigation action that the agent provides, parameterized by target_id.

**Key methods:**
- `get_provisions()`: Returns `[ProvisionSpec.fact("at_target", [target_id])]`
- `get_action_cost()`: Returns Euclidean distance to target (same as SpatialAction)
- `simulate_effect()`: Teleports agent to target location (same as SpatialAction)
- Navigation logic: Copied from `SpatialAction` (arrival thresholds, drift checks, nav agent management)

**Constructor parameters:**
- `target_id: String` (the target to navigate to)
- `target_location: GdPAILocationData` (resolved from target_id during planning)
- `max_interaction_distance: float`
- `max_drift_from_plan: float`

**How target_location is resolved:**
The planner needs to map `target_id` to a `GdPAILocationData`. This may require:
- A world lookup mechanism in the planner to resolve target_id → location
- Or the GoToAction receives the location directly when created

### 2. `examples/objects/holdable/pickup_interaction_action.gd` (Object-provided)

Pickup action that requires being at target and provides `held_item` binding.

**Key methods:**
- `get_requirements()`: Returns `[RequirementSpec.fact("at_target", [target_id])]`
- `get_provisions()`: Returns `[ProvisionSpec.binding("held_item", holdable_item.item_id)]`
- `get_preconditions()`: Requires empty hands (same as original PickupAction)
- `simulate_effect()`: Sets `held_item` property
- Interaction logic: Only handles pickup, no navigation

**Constructor parameters:**
- `holdable_item: HoldableObject`
- `target_id: String` (str(get_instance_id()) of the holdable object)
- `pickup_duration: float`

### 3. `examples/objects/fruit_tree/shake_tree_interaction_action.gd` (Object-provided)

Shake action that requires being at target.

**Key methods:**
- `get_requirements()`: Returns `[RequirementSpec.fact("at_target", [target_id])]`
- `get_validity_checks()`: Same as original ShakeTreeAction (hunger, cooldown checks)
- `simulate_effect()`: Reduces hunger (placeholder approach)
- Interaction logic: Only handles shaking, no navigation

**Constructor parameters:**
- `fruit_tree: FruitTreeObject`
- `target_id: String` (str(get_instance_id()) of the tree)

**Important note:** Calculate `_sim_hunger_gain` lazily in `simulate_effect()` or use a fixed value to avoid initialization timing issues with `fruit_tree.drop_min_amount`.

## Files to Modify

### 1. Agent Behavior Config (e.g., `examples/behaviors/hunger/hunger_behavior_config.gd`)

Add the agent's GoToAction to the action set:

```gdscript
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	updaters: Array[PropertyUpdater],
) -> void:
	goals.append(HungerGoal.new())
	actions.append(EatHeldFoodAction.new(hunger_restored_by_item, eat_duration))
	# Add agent's GoToAction (parameterized during planning)
	actions.append(GoToAction.new())  # May need different initialization pattern
	updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
```

**Open question:** How does the agent's GoToAction get parameterized with the correct target_id during planning? The planner needs to:
- Recognize that an object action requires `at_target(target_id)`
- Instantiate the agent's GoToAction with that specific target_id
- This may require planner support for dynamic action instantiation

### 2. `examples/objects/holdable/holdable_object.gd`

Change `get_provided_actions()` to return only PickupInteractionAction (no GoToAction):

```gdscript
func get_provided_actions() -> Array[Action]:
	var target_id: String = str(get_instance_id())
	return [PickupInteractionAction.new(self, target_id)]
```

### 3. `examples/objects/food/food_object.gd`

Same change as HoldableObject (since it extends HoldableObject).

### 4. `examples/objects/fruit_tree/fruit_tree_object.gd`

Change `get_provided_actions()` to return only ShakeTreeInteractionAction (no GoToAction):

```gdscript
func get_provided_actions() -> Array[Action]:
	var target_id: String = str(get_instance_id())
	return [ShakeTreeInteractionAction.new(self, target_id)]
```

## Implementation Details

### Target ID Generation

Use `str(get_instance_id())` to generate unique target IDs for objects. This ensures each object has a distinct identifier for provision/requirement matching.

### Target Location Resolution

The agent's GoToAction needs to resolve `target_id` to a `GdPAILocationData` during planning. Options:
1. **World lookup in planner**: Planner maintains a mapping of target_id → location_data
2. **Dynamic action instantiation**: Planner creates GoToAction instances with resolved location when needed
3. **Action receives location during planning**: GoToAction.get_action_cost() receives world_state and can look up location

This is a key architectural decision that affects planner implementation.

### Navigation Logic

Copy the navigation logic from `SpatialAction` to `GoToAction`:
- Arrival thresholds (ARRIVAL_THRESHOLD_2D, ARRIVAL_THRESHOLD_3D)
- Drift checking (max_drift_from_plan)
- Nav agent management (find_nav_agent, target_position updates)
- Movement tracking (prior_positions, time_elapsed)

### Cost Estimation

`GoToAction.get_action_cost()` should return the Euclidean distance between agent and target, same as `SpatialAction`. This requires the GoToAction to have access to the target's location data.

### Interaction Timing

Interaction actions should have a duration parameter (e.g., `pickup_duration`, `shake_duration`) for their cost estimation.

## Open Questions

### Dynamic Action Instantiation

How does the planner create parameterized GoToAction instances during planning? The current planner expects a fixed set of actions. Options:
1. **Template pattern**: Agent provides a GoToAction template that the planner clones with parameters
2. **Factory pattern**: Agent provides a factory that creates GoToAction instances on demand
3. **Planner enhancement**: Planner supports dynamic action creation with parameters

### Target Location Lookup

How does GoToAction resolve target_id to GdPAILocationData? Options:
1. Planner passes target_location to GoToAction during cost estimation
2. GoToAction looks up location from world_state using target_id
3. Objects include location_data in their action requirements

## Potential Issues

### Initialization Timing

The `ShakeTreeInteractionAction` constructor may encounter nil reference issues when accessing `fruit_tree.drop_min_amount`. Solution:
- Use a fixed default value for `_sim_hunger_gain`
- Calculate lazily in `simulate_effect()` with additional null checks
- Or accept the placeholder approach for tree shaking (as documented in PRECONDITIONS_REQUIREMENTS_PROVISIONS_PLAN.md)

### Action Titles

Ensure interaction actions keep the same titles as their SpatialAction counterparts:
- `PickupInteractionAction.get_title()` → "Pick Up Item"
- `ShakeTreeInteractionAction.get_title()` → "Shake Tree"

This maintains compatibility with existing tests.

## Testing Strategy

1. **Unit tests**: Test that GoToAction provides correct provisions and cost
2. **Integration tests**: Test that planner chains Agent GoTo → Object Interaction correctly
3. **Existing tests**: Ensure hunger example smoke test passes with new actions

## Summary

The compositional approach separates concerns cleanly:
- **Agent's GoToAction**: Pure navigation, provides location fact
- **Object interaction actions**: Pure domain logic, require location fact
- **Planner**: Naturally chains through requirements using backward chaining

This eliminates the need for `SpatialAction` to bundle navigation with interaction logic, making actions more reusable and composable. The agent owns navigation capability, and objects declare their location dependencies.

**Key difference from SpatialAction**: SpatialAction bundles navigation per-object, while the compositional approach centralizes navigation in the agent and uses requirements/provisions to express location dependencies.
