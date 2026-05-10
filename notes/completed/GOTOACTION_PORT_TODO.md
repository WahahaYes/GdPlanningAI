# GoToAction Port TODO

## Objective
Port examples from SpatialAction pattern to GoToAction pattern and phase out SpatialAction.

## Pattern Comparison

**Old SpatialAction Pattern:**
- `ShakeTreeAction` = navigation + shake interaction (single bundled action)
- `PickupAction` = navigation + pickup interaction (single bundled action)
- Actions extend `SpatialAction` which handles navigation internally

**New GoToAction Pattern:**
- `GoToAction` (navigation) → `ShakeTreeInteractionAction` (shake interaction)
- `GoToAction` (navigation) → `PickupInteractionAction` (pickup interaction)
- Navigation is a separate action that chains with interaction actions
- Interaction actions require `at_target` location provision from GoToAction
- Planner automatically chains GoTo → Interaction based on requirements/provisions

## Already Completed (Committed)
- GoToAction created with wildcard `at_target` provision
- Planner updated to handle wildcard bindings during planning
- Array support added to VariantSnapshot for multi-object bindings
- PickupInteractionAction created
- HoldableObject updated to provide PickupInteractionAction
- GoToAction added to hunger behavior config (partially)

## Remaining Tasks

### 1. Update FoodObject
- Change `get_provided_actions()` to return `PickupInteractionAction` instead of `PickupAction`
- File: `examples/objects/food/food_object.gd`

### 2. Create ShakeTreeInteractionAction
- Create new file: `examples/objects/fruit_tree/shake_tree_interaction_action.gd`
- Should extend `Action` (not SpatialAction)
- Should require `at_target` location via `get_requirements()`
- Should handle shake interaction logic (no navigation)
- Similar pattern to PickupInteractionAction

### 3. Update FruitTreeObject
- Change `get_provided_actions()` to return `ShakeTreeInteractionAction` instead of `ShakeTreeAction`
- File: `examples/objects/fruit_tree/fruit_tree_object.gd`

### 4. Update Hunger Behavior Config
- Ensure GoToAction is in the actions array
- File: `examples/behaviors/hunger/hunger_behavior_config.gd`

### 5. Update Test Expectations
- Test currently expects single actions: "Shake Tree", "Pick Up Item"
- New pattern produces chains: "Go To" → "Shake Tree", "Go To" → "Pick Up Item"
- Update `test/integration/test_hunger_example_smoke.gd` to expect action chains
- Update assertions to check for `shake_plan[0].get_title() == "Go To"` and `shake_plan[1].get_title() == "Shake Tree"`

### 6. Delete Old Action Files
- Delete `examples/objects/holdable/pickup_action.gd`
- Delete `examples/objects/fruit_tree/shake_tree_action.gd`
- Delete corresponding `.uid` files

### 7. Update WanderAction
- Change to use `SpatialAction.find_nav_agent()` or extract to utility
- Change to use `SpatialAction.ARRIVAL_THRESHOLD_2D/3D` constants
- File: `examples/behaviors/wander/wander_action.gd`
- Note: WanderAction doesn't fit the GoTo pattern well since it wanders to random locations, not specific objects

### 8. Deprecate or Remove SpatialAction
- Add deprecation warning if keeping for backward compatibility
- Or remove if no longer needed after port is complete
- File: `addons/GdPlanningAI/scripts/refcounteds/spatial_action.gd`

### 9. Test Verification
- Run `make test-godot` to verify all tests pass
- Run `make build-release` to rebuild Rust binary
- Test hunger example interactively to verify GoTo → Interaction chaining works

## Key Implementation Notes

### Interaction Action Pattern
Interaction actions should:
1. Extend `Action` (not `SpatialAction`)
2. Require `at_target` location via `get_requirements()` returning `RequirementSpec.fact("at_target", [location_data])`
3. Not handle navigation - GoToAction handles that
4. Handle only the interaction logic (shake, pickup, etc.)
5. Be provided by world objects via `get_provided_actions()`

### Planner Chaining
The planner automatically chains actions when:
- Action A requires a fact (e.g., `at_target`)
- Action B provides that fact as a wildcard provision
- Planner selects B → A to satisfy the requirement

### Test Environment
The test creates a world node and adds objects to the scene. The planner discovers object-provided actions via:
- `GdPAIAgent._collect_worldly_actions()` calls `world_node.get_world_state()`
- World node collects objects from "GdPAIObjectData" group
- Each object's `get_provided_actions()` is called to collect actions
