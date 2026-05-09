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

## Fact Wildcard Provision

### What is a Fact Wildcard?

A `ProvisionSpec.fact_wildcard(fact_name)` is a provision that can satisfy ANY requirement with the same `fact_name`, regardless of the requirement's arguments. This is different from regular fact provisions which require exact argument match.

**Comparison with bindings:**

| Type | Provision | Requirement | Direction |
|------|-----------|-------------|-----------|
| **Binding** | `binding("held_item", "banana")` | `binding_exists("held_item")` | Provision → Requirement (provision has concrete value) |
| **Fact** | `fact("at_target", [loc1])` | `fact("at_target", [loc1])` | Exact match required |
| **Fact Wildcard** | `fact_wildcard("at_target")` | `fact("at_target", [loc1])` | Requirement → Provision (provision captures args from requirement) |

**Key distinction:**
- For bindings and regular facts: The provision already knows its value/args when created
- For fact wildcards: The provision has no args, but captures them from the requirement it satisfies

### Why Fact Wildcards Enable Compositional GoTo

The fact wildcard allows a **single GoToAction instance** to satisfy different location requirements:

1. **PickupInteractionAction** requires `fact("at_target", [food_location])`
2. **ShakeTreeInteractionAction** requires `fact("at_target", [tree_location])`
3. **Agent's GoToAction** provides `fact_wildcard("at_target")`

When the planner chains actions:
- If PickupInteractionAction is selected, it introduces requirement `at_target(food_location)`
- GoToAction's wildcard provision matches this requirement
- GoToAction captures `food_location` as its target
- The planner chains: `GoToAction(food_location) → PickupInteractionAction → EatHeldFoodAction`

This eliminates the need for per-object GoToAction instances. The agent provides one generic GoToAction that dynamically adapts to whichever location requirement it needs to satisfy.

### Implementation Status

**Completed:**
- `ProvisionSpecFactWildcard` class in GDScript
- `FactWildcard` variant in Rust `ProvisionSpec` enum
- Matching logic in `provision_satisfies_requirement_in_context`
- Integration tests verifying wildcard matching

**Remaining:**
- **Dynamic action parameterization mechanism**: When a wildcard provision matches a requirement, the planner needs to:
  1. Capture the requirement's arguments (e.g., `[food_location]`)
  2. Store these bindings in the `PlanBranch` during planning
  3. Include bindings in `PlanResult` so GDScript can access them
  4. Pass bindings to actions during execution (e.g., set GoToAction's `target_location`)

### Proposed Planner Changes

To enable dynamic action parameterization:

1. **Add `bound_fact_args` to `PlanBranch`**:
   ```rust
   bound_fact_args: HashMap<(String, usize), Vec<VariantSnapshot>>
   // Key: (fact_name, action_index), Value: args from requirement
   ```

2. **Update `update_open_needs`** to capture fact arg bindings when wildcard provisions match requirements

3. **Extend `PlanResult`** to include action parameter bindings:
   ```rust
   pub struct PlanResult {
       // ... existing fields
       pub action_bindings: HashMap<usize, Vec<VariantSnapshot>>,
       // action_index → bound arguments
   }
   ```

4. **Update GDScript bridge** to pass bindings to actions during execution

5. **Update GoToAction** to accept location from plan result bindings

This approach mirrors the existing `bound_provisions` mechanism used for binding requirements, extending it to handle fact argument binding for wildcards.

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
