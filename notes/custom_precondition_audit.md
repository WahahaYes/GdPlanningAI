# Custom Precondition Audit & Refactoring Plan

**Date**: 2025-07-25
**Context**: Analysis of opaque custom preconditions hindering backward-chaining planner discovery

---

## The Problem

Custom preconditions (`Precondition.custom(callable)`) are **black boxes** to the Rust planner:
- Cannot be evaluated during candidate discovery without a GDScript callback round-trip
- Planner cannot know *which actions* might satisfy them
- Forces users to manually chain actions they already know work

The planner has two discovery paths:
1. **Symbolic** (Requirements ↔ Provisions) — fully visible, zero callbacks
2. **Simulation** (Builtin Preconditions) — evaluated on simulated snapshots in Rust, zero callbacks

Custom preconditions fall into neither — they require main-thread callbacks *during* discovery, blocking the search.

---

## Current Custom Precondition Usages (4 total)

| File | Type | What It Checks | Refactorable? |
|------|------|----------------|---------------|
| `shake_tree_action.gd` | Validity check | `fruit_tree.is_on_cooldown` (property on specific object instance) | **Yes** → WorldObjectProxy property |
| `wander_goal.gd` | Desired state | Agent position moved ≥ 16px from start | **Yes** → WorldObjectProxy property on agent's own location data |
| `eat_held_food_action.gd` | Precondition | `held_item` exists in `hunger_restored_by_item` dict | **Partial** — needs binding-aware builtin |
| `maintain_fire_goal.gd` | Desired state | Any `CampfireObject` has `current_fuel ≥ desired_fuel_level` | **Yes** → WorldObjectProxy property |

---

## Proposed Solution: WorldObjectProxy Builtin Preconditions

Add new `PreconditionTarget::WorldObjectProxy { group, property }` that evaluates directly on simulated `SimObjectProxy` snapshots in Rust.

### New Builtin Operations
```gdscript
Precondition.world_object_property_equal_to(group, property, value)
Precondition.world_object_property_not_equal_to(group, property, value)
Precondition.world_object_property_greater_than(group, property, value)
Precondition.world_object_property_geq_than(group, property, value)
Precondition.world_object_property_less_than(group, property, value)
Precondition.world_object_property_leq_than(group, property, value)
Precondition.world_object_has_property(group, property)
```

### Refactoring Each Usage

#### 1. `ShakeTreeAction` — Validity Check
**Before:**
```gdscript
var tree_not_on_cooldown = func(_bb, _ws) -> bool:
    if is_instance_valid(fruit_tree):
        return not fruit_tree.is_on_cooldown
    return false
checks.append(Precondition.custom(tree_not_on_cooldown))
```

**After:**
```gdscript
# Validity checks can use object-dependent preconditions
checks.append(Precondition.check_is_object_valid(fruit_tree))
# But for property on that specific object:
# Need: Precondition.object_property_equal_to(object_ref, property, value)
# OR: Precondition.world_object_property_equal_to("FruitTreeObject", "is_on_cooldown", false)
# Note: group-based matches ALL objects in group; may need instance-specific variant
```

#### 2. `WanderGoal` — Desired State
**Before:**
```gdscript
var far_enough = func(blackboard, _ws) -> bool:
    var sim_location = blackboard.get_proxy_in_group("GdPAILocationData")
    return (sim_location.get_property("position") - agent_position).length() > req_distance
return [Precondition.custom(far_enough)]
```

**After:**
```gdscript
# Agent's own location data is in group "GdPAILocationData"
# Want: distance from original position > threshold
# This is a computed property, not a direct object property
# Option A: Virtual property on world blackboard
# Option B: Add "distance_from_origin" as simulated property in WanderAction.simulate_effect
```

#### 3. `EatHeldFoodAction` — Precondition
**Before:**
```gdscript
preconds.append(Precondition.custom(Callable(self, "_is_holding_food")))
```

**After:** This checks a *binding-dependent* condition (held_item must be in action's internal dict). Not directly expressible as world object property. Could add:
```gdscript
Precondition.binding_in_set("held_item", "allowed_food_items")
```
Where `allowed_food_items` is a provision fact provided by the action itself.

#### 4. `MaintainFireGoal` — Desired State ⭐ **Primary Target**
**Before:**
```gdscript
var check = func(_bb, world) -> bool:
    for campfire in world.get_proxies_in_group("CampfireObject"):
        if campfire.get_property("current_fuel") >= desired_fuel_level:
            return true
    return false
return [Precondition.custom(check)]
```

**After:**
```gdscript
return [Precondition.world_object_property_geq_than("CampfireObject", "current_fuel", desired_fuel_level)]
```

---

## Implementation Plan

### Phase 1: Core Rust Changes
1. `precondition.rs` — Add `WorldObjectProxy { group: String, property: String }` to `PreconditionTarget`
2. `plan_types.rs` — Add `eval_builtin_on_snapshot` handling for proxy target (iterate `world.objects` by group)
3. `precondition.gd` — Add static constructors

### Phase 2: Refactor Examples
1. `maintain_fire_goal.gd` — Use `world_object_property_geq_than`
2. `shake_tree_action.gd` — Use `world_object_property_equal_to` (or new instance-specific variant)
3. `wander_goal.gd` — Requires virtual property or simulate_effect augmentation
4. `eat_held_food_action.gd` — Add `binding_in_set` requirement/provision pattern

### Phase 3: Virtual Property System (Optional but Powerful)
Register computed properties on world blackboard:
```gdscript
world_bb.register_virtual_property("nearest_campfire_fuel", func(world):
    # compute max fuel across all campfires
)
```
Then `Precondition.world_state_property_geq_than("nearest_campfire_fuel", 60)`

---

## Files to Modify

### Rust
- `addons/GdPlanningAI/rust/src/precondition.rs`
- `addons/GdPlanningAI/rust/src/plan_types.rs`
- `addons/GdPlanningAI/rust/src/precondition.rs` (PreconditionOp enum)

### GDScript
- `addons/GdPlanningAI/scripts/refcounteds/precondition.gd`
- `examples/behaviors/campfire/maintain_fire_goal.gd`
- `examples/objects/fruit_tree/shake_tree_action.gd`
- `examples/behaviors/wander/wander_goal.gd`
- `examples/behaviors/campfire/eat_held_food_action.gd`

---

## Testing

Add integration test: `test_maintain_fire_goal_uses_builtin_precondition.gd`
- Verify planner discovers `AddFuelAction` without custom callback
- Verify plan cost matches Dijkstra (no callback overhead)

---

## Notes

- `ShakeTreeAction` validity check uses `is_instance_valid(fruit_tree)` — the object-specific check. Group-based proxy checks all objects in group. May need `Precondition.object_property_*` for instance-specific.
- `WanderGoal` distance check is inherently relative to *original* position, not a stored property. Best solved by having `WanderAction.simulate_effect` record origin position as a simulated property.