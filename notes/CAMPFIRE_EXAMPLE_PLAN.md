# GdPlanningAI — Campfire Tending Example

> **Last updated:** 2026-05-10 — rewritten for GoToAction architecture.
> Existing components (`CampfireBehaviorConfig`, `MaintainFireGoal`, `EatHeldFoodAction`,
> `DropItemAction`, `PickupAction`, `HoldableObject`, `HungerGoal`, `HungerBehaviorConfig`,
> `HungerPropertyUpdater`) are already implemented and are referenced rather than re-specified.

---

## Agreed v1 Implementation Scope

- **Primary target:** Implement **both 2D and 3D demo scenes**. All gameplay logic (goals, actions, object data, spawner) is dimension-agnostic and shared. Only the prefab/scene layer differs between 2D and 3D — this showcases that the planning system is completely independent of the rendering dimension.
- **Fire maintenance reward:** `MaintainFireGoal` uses a **simple exported/parameterized reward value** in v1 rather than computing a dynamic reward from fire fuel.
- **Fire fuel access:** Campfire-related actions may **query the referenced campfire object directly** for runtime/planning validity and cost checks in the first pass.
- **Inventory flexibility:** `DropItemAction` (already implemented) lets agents recover from plans where they must switch from holding food to holding wood.
- **Eating action ownership:** Resource objects provide pickup/interaction actions, but eating is an **agent-provided action**. `EatHeldFoodAction` (already implemented) lives on `HungerBehaviorConfig` and restores hunger from a configurable per-item dictionary keyed by held item id.
- **Object data responsibility:** `GdPAIObjectData` remains focused on **planning-system integration only**. Any campfire fuel visuals, labels, or scene presentation live in separate scene nodes/scripts.
- **UI scope:** Keep the initial UI/visual feedback **minimal** and prioritize a working end-to-end implementation.

---

## Goals

This example demonstrates **maintenance/proactive planning with resource transformation** — a planning pattern where agents must maintain shared resources while also preparing food through a cooking process.

**Key Learning Outcomes:**
1. **Competing priorities** — balance personal needs (hunger) vs group needs (fire maintenance)
2. **Proactive planning** — gather fuel before fire dies, not after
3. **Carrying/inventory state** — track held items (wood/potato/cooked_potato) via blackboard
4. **Multi-step preparation chains** — must gather resources AND cook them before consumption
5. **Resource transformation** — raw potatoes → cooked potatoes via fire interaction
6. **Time-critical actions** — fire fuel decays, creating urgency
7. **Threshold-based goals** — goal reward scales as resource depletes
8. **Dynamic respawning** — potatoes respawn at randomized locations

**Note for v1:** Uses a **parameterized fixed reward** for fire maintenance. All gameplay code is shared between 2D and 3D — the planning system is dimension-agnostic.

---

## Design Overview

### The Scenario

Agents gather around a campfire that tracks its own fuel level (attached to the campfire object). The fire's fuel decays over time. Agents must:
1. **Gather wood** from wood piles to maintain the fire
2. **Dig up raw potatoes** from randomized ground spawn locations
3. **Cook potatoes** at the fire (requires fire fuel > threshold)
4. **Eat cooked potatoes** to satisfy hunger
5. **Drop held items when needed** to switch tasks

**Fail state:** If fire fuel reaches 0, cooking is blocked until fire is refueled.

**Success pattern:** Agents proactively maintain fuel levels AND cook food before hunger becomes critical.

---

## Architecture: GoToAction Chaining Pattern

The planner uses a **compositional navigation pattern** where `GoToAction` (navigation) chains with interaction actions via `at_target` requirements/provisions:

```
GoToAction                    PickUpWoodAction
  provides: at_target (wildcard)  requires: at_target [wood_pile_location]
                                  preconditions: held_item == ""
                                  simulate_effect: held_item = "wood"
```

The planner automatically chains `GoToAction → PickUpWoodAction` because `GoToAction`'s wildcard `at_target` provision satisfies `PickUpWoodAction`'s `at_target` requirement.

All new interaction actions follow this pattern:
- Extend `Action` (not `SpatialAction`)
- Declare `get_requirements()` returning `RequirementSpec.fact("at_target", [location_data])`
- Do NOT handle navigation — `GoToAction` handles that
- `get_validity_checks()` only checks object reference validity
- `get_preconditions()` checks state conditions (held_item values, etc.)

---

## Reused Infrastructure

| Existing System | How We Use It |
|---|---|
| `GoToAction` | Agent-provided navigation; chains with all interaction actions via `at_target` |
| `HungerBehaviorConfig` | Provides `HungerGoal`, `GoToAction`, `EatHeldFoodAction`, `HungerPropertyUpdater` |
| `CampfireBehaviorConfig` | Provides `MaintainFireGoal`, `DropItemAction` |
| `HoldableObject` / `PickupAction` | Reusable pickup contract (used by food objects; wood/potato use their own interaction actions) |
| `DropItemAction` | Agent-provided self action to clear `held_item` |
| `EatHeldFoodAction` | Agent-provided self action; requires `held_item` binding, restores hunger from dictionary |
| `GdPAIObjectData` | `WoodPileObject`, `PotatoObject`, `CampfireObject` provide interaction actions |
| `PropertyUpdater` | `HungerPropertyUpdater` decays hunger over time |
| `Precondition` builtins | `agent_has_property`, `agent_property_equal_to`, `agent_property_not_equal_to`, `check_is_object_valid`, `custom`, etc. |

**Net new code:** Three object types (`WoodPileObject`, `CampfireObject`, `PotatoObject`), four interaction actions (`PickUpWoodAction`, `AddFuelAction`, `CookPotatoAction`, `DigPotatoAction`), one spawner system (`PotatoSpawner`), and the demo scene + prefabs.

---

## New Components

### 1. Wood Pile — `examples/objects/wood_pile/`

**`wood_pile_object.gd`** (`WoodPileObject extends GdPAIObjectData`)
```gdscript
class_name WoodPileObject
extends GdPAIObjectData

@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData


func get_group_labels() -> Array[String]:
    return ["WoodPileObject", "GdPAIObjectData"]


func get_provided_actions() -> Array[Action]:
    return [PickUpWoodAction.new(location_data, interactable_attribs)]


func get_sim_properties() -> Dictionary:
    return {}
```

**`pick_up_wood_action.gd`** (`PickUpWoodAction extends Action`)
```gdscript
class_name PickUpWoodAction
extends Action

var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable


func _init(
    p_object_location: GdPAILocationData,
    p_interactable_attribs: GdPAIInteractable,
) -> void:
    object_location = p_object_location
    interactable_attribs = p_interactable_attribs


func get_validity_checks() -> Array[Precondition]:
    return [
        Precondition.check_is_object_valid(object_location),
        Precondition.check_is_object_valid(interactable_attribs),
    ]


func get_preconditions() -> Array[Precondition]:
    return [Precondition.agent_property_equal_to("held_item", "")]


func get_requirements() -> Array[RequirementSpec]:
    if object_location != null:
        return [RequirementSpec.fact("at_target", [object_location])]
    return []


func get_action_cost(
    _agent_blackboard: GdPAIBlackboard,
    _world_state: GdPAIBlackboard,
) -> float:
    return 0.5


func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    _world_state: GdPAIBlackboard,
) -> void:
    agent_blackboard.set_property("held_item", "wood")


func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
    agent.blackboard.set_property("held_item", "wood")
    return Action.Status.SUCCESS


func get_title() -> String:
    return "Pick Up Wood"


func get_description() -> String:
    return "Pick up wood from a wood pile (requires GoToAction for navigation)."
```

---

### 2. Campfire — `examples/objects/campfire/`

**`campfire_object.gd`** (`CampfireObject extends GdPAIObjectData`)
```gdscript
class_name CampfireObject
extends GdPAIObjectData

@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData
@export var fuel_per_wood: float = 30.0
@export var min_fuel_to_cook: float = 20.0

var current_fuel: float = 100.0
var fuel_decay_rate: float = 3.0


func get_group_labels() -> Array[String]:
    return ["CampfireObject", "GdPAIObjectData"]


func get_provided_actions() -> Array[Action]:
    return [
        AddFuelAction.new(self, location_data, interactable_attribs, fuel_per_wood),
        CookPotatoAction.new(self, location_data, interactable_attribs, min_fuel_to_cook),
    ]


func get_sim_properties() -> Dictionary:
    return {
        "fuel_per_wood": fuel_per_wood,
        "current_fuel": current_fuel,
        "min_fuel_to_cook": min_fuel_to_cook,
    }
```

**`add_fuel_action.gd`** (`AddFuelAction extends Action`)
```gdscript
class_name AddFuelAction
extends Action

const ADD_FUEL_DURATION: float = 1.0

var campfire_ref: CampfireObject
var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable
var fuel_per_wood: float


func _init(
    p_campfire: CampfireObject,
    p_object_location: GdPAILocationData,
    p_interactable_attribs: GdPAIInteractable,
    p_fuel_per_wood: float,
) -> void:
    campfire_ref = p_campfire
    object_location = p_object_location
    interactable_attribs = p_interactable_attribs
    fuel_per_wood = p_fuel_per_wood


func get_validity_checks() -> Array[Precondition]:
    return [
        Precondition.check_is_object_valid(campfire_ref),
        Precondition.check_is_object_valid(object_location),
        Precondition.check_is_object_valid(interactable_attribs),
    ]


func get_preconditions() -> Array[Precondition]:
    return [Precondition.agent_property_equal_to("held_item", "wood")]


func get_requirements() -> Array[RequirementSpec]:
    if object_location != null:
        return [RequirementSpec.fact("at_target", [object_location])]
    return []


func get_action_cost(
    _agent_blackboard: GdPAIBlackboard,
    _world_state: GdPAIBlackboard,
) -> float:
    if not is_instance_valid(campfire_ref):
        return INF
    if campfire_ref.current_fuel >= 100.0:
        return INF
    return ADD_FUEL_DURATION


func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    _world_state: GdPAIBlackboard,
) -> void:
    agent_blackboard.set_property("held_item", "")


func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
    set_state(agent, "add_fuel_elapsed", 0.0)
    return Action.Status.SUCCESS


func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
    if not is_instance_valid(campfire_ref):
        return Action.Status.FAILURE

    var elapsed: float = get_state(agent, "add_fuel_elapsed") + delta
    set_state(agent, "add_fuel_elapsed", elapsed)

    if elapsed >= ADD_FUEL_DURATION:
        campfire_ref.current_fuel = min(100.0, campfire_ref.current_fuel + fuel_per_wood)
        agent.blackboard.set_property("held_item", "")
        return Action.Status.SUCCESS

    return Action.Status.RUNNING


func post_perform_action(agent: GdPAIAgent) -> Action.Status:
    erase_state(agent, "add_fuel_elapsed")
    return Action.Status.SUCCESS


func get_title() -> String:
    return "Add Fuel"


func get_description() -> String:
    return "Add wood to the campfire to increase its fuel level."
```

**`cook_potato_action.gd`** (`CookPotatoAction extends Action`)
```gdscript
class_name CookPotatoAction
extends Action

const COOK_DURATION: float = 2.0

var campfire_ref: CampfireObject
var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable
var min_fuel_to_cook: float


func _init(
    p_campfire: CampfireObject,
    p_object_location: GdPAILocationData,
    p_interactable_attribs: GdPAIInteractable,
    p_min_fuel_to_cook: float,
) -> void:
    campfire_ref = p_campfire
    object_location = p_object_location
    interactable_attribs = p_interactable_attribs
    min_fuel_to_cook = p_min_fuel_to_cook


func get_validity_checks() -> Array[Precondition]:
    return [
        Precondition.check_is_object_valid(campfire_ref),
        Precondition.check_is_object_valid(object_location),
        Precondition.check_is_object_valid(interactable_attribs),
    ]


func get_preconditions() -> Array[Precondition]:
    return [Precondition.agent_property_equal_to("held_item", "potato")]


func get_requirements() -> Array[RequirementSpec]:
    if object_location != null:
        return [RequirementSpec.fact("at_target", [object_location])]
    return []


func get_action_cost(
    _agent_blackboard: GdPAIBlackboard,
    _world_state: GdPAIBlackboard,
) -> float:
    if not is_instance_valid(campfire_ref):
        return INF
    if campfire_ref.current_fuel < min_fuel_to_cook:
        return INF
    return COOK_DURATION


func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    _world_state: GdPAIBlackboard,
) -> void:
    agent_blackboard.set_property("held_item", "cooked_potato")


func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
    set_state(agent, "cook_elapsed", 0.0)
    return Action.Status.SUCCESS


func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
    if not is_instance_valid(campfire_ref):
        return Action.Status.FAILURE
    if campfire_ref.current_fuel < min_fuel_to_cook:
        return Action.Status.FAILURE

    var elapsed: float = get_state(agent, "cook_elapsed") + delta
    set_state(agent, "cook_elapsed", elapsed)

    if elapsed >= COOK_DURATION:
        agent.blackboard.set_property("held_item", "cooked_potato")
        return Action.Status.SUCCESS

    return Action.Status.RUNNING


func post_perform_action(agent: GdPAIAgent) -> Action.Status:
    erase_state(agent, "cook_elapsed")
    return Action.Status.SUCCESS


func get_title() -> String:
    return "Cook Potato"


func get_description() -> String:
    return "Cook a raw potato at the campfire (requires minimum fuel level)."
```

---

### 3. Potato — `examples/objects/potato/`

**`potato_object.gd`** (`PotatoObject extends GdPAIObjectData`)
```gdscript
class_name PotatoObject
extends GdPAIObjectData

@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData


func get_group_labels() -> Array[String]:
    return ["PotatoObject", "GdPAIObjectData"]


func get_provided_actions() -> Array[Action]:
    return [DigPotatoAction.new(location_data, interactable_attribs, self)]


func get_sim_properties() -> Dictionary:
    return {}
```

**`dig_potato_action.gd`** (`DigPotatoAction extends Action`)
```gdscript
class_name DigPotatoAction
extends Action

var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable
var potato_ref: PotatoObject


func _init(
    p_object_location: GdPAILocationData,
    p_interactable_attribs: GdPAIInteractable,
    p_potato_ref: PotatoObject,
) -> void:
    object_location = p_object_location
    interactable_attribs = p_interactable_attribs
    potato_ref = p_potato_ref


func get_validity_checks() -> Array[Precondition]:
    return [
        Precondition.check_is_object_valid(object_location),
        Precondition.check_is_object_valid(interactable_attribs),
        Precondition.check_is_object_valid(potato_ref),
    ]


func get_preconditions() -> Array[Precondition]:
    return [Precondition.agent_property_equal_to("held_item", "")]


func get_requirements() -> Array[RequirementSpec]:
    if object_location != null:
        return [RequirementSpec.fact("at_target", [object_location])]
    return []


func get_action_cost(
    _agent_blackboard: GdPAIBlackboard,
    _world_state: GdPAIBlackboard,
) -> float:
    return 0.7


func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    _world_state: GdPAIBlackboard,
) -> void:
    agent_blackboard.set_property("held_item", "potato")


func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
    if not is_instance_valid(potato_ref) or not is_instance_valid(potato_ref.entity):
        return Action.Status.FAILURE
    agent.blackboard.set_property("held_item", "potato")
    potato_ref.entity.queue_free()
    return Action.Status.SUCCESS


func get_title() -> String:
    return "Dig Potato"


func get_description() -> String:
    return "Dig up a raw potato from the ground (requires GoToAction for navigation)."
```

---

### 4. Potato Spawner — `examples/shared/systems/potato_spawner/`

**`potato_spawner.gd`** (`PotatoSpawner extends Node`)
```gdscript
class_name PotatoSpawner
extends Node

@export var potato_scene: PackedScene
@export var spawn_area: Rect2
@export var max_potatoes: int = 5
@export var respawn_time: float = 10.0

var _active_potatoes: Array[Node] = []
var _respawn_timer: float = 0.0


func _ready() -> void:
    for i in range(max_potatoes):
        _spawn_potato()


func _process(delta: float) -> void:
    _active_potatoes = _active_potatoes.filter(func(p): return is_instance_valid(p))

    _respawn_timer += delta
    if _active_potatoes.size() < max_potatoes and _respawn_timer >= respawn_time:
        _spawn_potato()
        _respawn_timer = 0.0


func _spawn_potato() -> void:
    if not potato_scene:
        return

    var potato: Node = potato_scene.instantiate()
    var random_pos: Vector2 = Vector2(
        randf_range(spawn_area.position.x, spawn_area.position.x + spawn_area.size.x),
        randf_range(spawn_area.position.y, spawn_area.position.y + spawn_area.size.y),
    )

    if potato is Node2D:
        potato.position = random_pos

    get_parent().add_child(potato)
    _active_potatoes.append(potato)
```

---

### 5. CampfireBehaviorConfig Update

The existing `CampfireBehaviorConfig` at `examples/behaviors/campfire/campfire_behavior_config.gd` needs one addition: `GoToAction` must be included so the agent can navigate. Currently only `HungerBehaviorConfig` adds `GoToAction`. Add it to `CampfireBehaviorConfig._populate()`:

```gdscript
func _populate(
    goals: Array[Goal],
    actions: Array[Action],
    _updaters: Array[PropertyUpdater],
) -> void:
    goals.append(MaintainFireGoal.new(fire_goal_reward, desired_fuel_level))
    actions.append(GoToAction.new())
    actions.append(DropItemAction.new(drop_duration))
```

---

## Planning Chains (GoToAction Pattern)

With the GoToAction architecture, the planner produces **action pairs** for every spatial interaction:

| Goal | Planned Chain |
|---|---|
| Eat food | `GoToAction` → `PickUpWoodAction` → `GoToAction` → `AddFuelAction` → `GoToAction` → `DigPotatoAction` → `GoToAction` → `CookPotatoAction` → `EatHeldFoodAction` |
| Maintain fire | `GoToAction` → `PickUpWoodAction` → `GoToAction` → `AddFuelAction` |

The planner automatically interleaves `GoToAction` instances because every interaction action requires `at_target`.

---

## Planning Scenarios Demonstrated

### Scenario 1: Full Cooking Chain
**State:** Hunger = 70, Fire fuel = 80, held_item = ""

**Expected plan:**
1. GoTo(potato) → Dig potato (held_item = "potato")
2. GoTo(campfire) → Cook potato (held_item = "cooked_potato")
3. Eat cooked potato (hunger -= 50, held_item = "")

Agent completes full resource transformation chain.

---

### Scenario 2: Fire Too Low to Cook
**State:** Hunger = 60, Fire fuel = 10, held_item = "potato"

**Expected plan:**
1. Drop potato (held_item = "")
2. GoTo(wood pile) → Pick up wood (held_item = "wood")
3. GoTo(campfire) → Add fuel (fire += 30, held_item = "")
4. GoTo(potato) → Dig potato (held_item = "potato")
5. GoTo(campfire) → Cook potato (held_item = "cooked_potato")
6. Eat cooked potato

Agent recognizes cooking is blocked and must refuel first.

---

### Scenario 3: Preemptive Fire Maintenance
**State:** Hunger = 20, Fire fuel = 35, held_item = ""

**Expected plan:**
1. GoTo(wood pile) → Pick up wood (held_item = "wood")
2. GoTo(campfire) → Add fuel (fire += 30 → 65, held_item = "")
3. GoTo(potato) → Dig potato (held_item = "potato")
4. GoTo(campfire) → Cook potato (held_item = "cooked_potato")
5. Eat cooked potato

Agent maintains fire proactively before it becomes critical.

---

### Scenario 4: Competing Priorities
**State:** Hunger = 95, Fire fuel = 15, held_item = ""

**Expected plan (hunger wins if reward > fire reward):**
1. GoTo(potato) → Dig potato
2. GoTo(campfire) → Cook potato
3. Eat cooked potato
4. Then: GoTo(wood pile) → Pick up wood → GoTo(campfire) → Add fuel

**Expected plan (fire wins if reward > hunger reward):**
1. GoTo(wood pile) → Pick up wood
2. GoTo(campfire) → Add fuel
3. Then: GoTo(potato) → Dig potato → GoTo(campfire) → Cook potato → Eat

Agent balances competing goals based on configured reward values.

---

## File Locations

**Already exists (no changes needed):**
- `examples/behaviors/campfire/campfire_behavior_config.gd` — minor update: add `GoToAction`
- `examples/behaviors/campfire/maintain_fire_goal.gd`
- `examples/behaviors/campfire/eat_held_food_action.gd`
- `examples/behaviors/hunger/` — entire directory
- `examples/objects/holdable/` — entire directory

**New files to create:**
- `examples/objects/wood_pile/wood_pile_object.gd`
- `examples/objects/wood_pile/pick_up_wood_action.gd`
- `examples/objects/campfire/campfire_object.gd`
- `examples/objects/campfire/add_fuel_action.gd`
- `examples/objects/campfire/cook_potato_action.gd`
- `examples/objects/potato/potato_object.gd`
- `examples/objects/potato/dig_potato_action.gd`
- `examples/shared/systems/potato_spawner/potato_spawner.gd`
- `examples/campfire_2d.tscn`
- `examples/campfire_3d.tscn`

**New 2D prefabs:**
- `examples/source_2d/prefabs/wood_pile_2d.tscn`
- `examples/source_2d/prefabs/campfire_2d.tscn`
- `examples/source_2d/prefabs/potato_2d.tscn`
- `examples/source_2d/prefabs/agent_2d.tscn`

**New 3D prefabs:**
- `examples/source_3d/prefabs/wood_pile_3d.tscn`
- `examples/source_3d/prefabs/campfire_3d.tscn`
- `examples/source_3d/prefabs/potato_3d.tscn`
- `examples/source_3d/prefabs/agent_3d.tscn`

---

## Prefab Prototypes (Primitive Shapes)

All prototype prefabs use **Godot primitive nodes only** — no imported assets, no custom textures.
This makes it trivial to swap in real art later by replacing the primitive children while keeping
the script, collision, and label structure intact.

All gameplay scripts are **shared between 2D and 3D**. The only difference is the node hierarchy
in each prefab. This showcases that the planning system is completely dimension-agnostic.

### 2D Prefabs

#### Agent — `examples/source_2d/prefabs/agent_2d.tscn`

```
CharacterBody2D (root)
├── CollisionShape2D (CircleShape2D, radius ~16)
├── NavigationAgent2D
├── ColorRect (24×24, centered, color: blue or per-agent tint)
├── Label ("Agent" above head, existing AgentDebugLabel script)
├── GdPAIAgent (script)
├── GdPAILocationData (script)
└── HungerBehaviorConfig + CampfireBehaviorConfig (exported children)
```

#### Wood Pile — `examples/source_2d/prefabs/wood_pile_2d.tscn`

```
Node2D (root)
├── CollisionShape2D (RectangleShape2D, ~32×16, for interaction radius)
├── ColorRect (32×16, brown #8B6914, centered)
├── Label ("Wood Pile" above)
├── GdPAIObjectData → WoodPileObject (script)
├── GdPAIInteractable (script)
└── GdPAILocationData (script)
```

#### Campfire — `examples/source_2d/prefabs/campfire_2d.tscn`

```
Node2D (root)
├── CollisionShape2D (CircleShape2D, radius ~24, for interaction radius)
├── Circle (radius 20, color: orange-red #E85D3F, centered)     ← fire glow
├── ColorRect (8×16, brown #5C3A1E, offset upward)              ← log 1
├── ColorRect (8×16, brown #5C3A1E, rotated ~30°, offset up)    ← log 2
├── Label ("Campfire (100%)" above, updated by scene script)
├── GdPAIObjectData → CampfireObject (script)
├── GdPAIInteractable (script)
└── GdPAILocationData (script)
```

#### Potato — `examples/source_2d/prefabs/potato_2d.tscn`

```
Node2D (root)
├── CollisionShape2D (CircleShape2D, radius ~10, for interaction radius)
├── Circle (radius 8, color: tan #D2B48C, centered)
├── Label ("Potato" above)
├── GdPAIObjectData → PotatoObject (script)
├── GdPAIInteractable (script)
└── GdPAILocationData (script)
```

### 3D Prefabs

Same scripts, same structure — just 3D node types and CSG primitives.

#### Agent — `examples/source_3d/prefabs/agent_3d.tscn`

```
CharacterBody3D (root)
├── CollisionShape3D (CylinderShape3D, radius ~0.5, height ~1.0)
├── NavigationAgent3D
├── CSGBox3D (0.8×0.8×1.6, centered, material: blue or per-agent tint)
├── Label3D ("Agent" above head, existing AgentDebugLabel script)
├── GdPAIAgent (script)
├── GdPAILocationData (script)
└── HungerBehaviorConfig + CampfireBehaviorConfig (exported children)
```

#### Wood Pile — `examples/source_3d/prefabs/wood_pile_3d.tscn`

```
Node3D (root)
├── CollisionShape3D (BoxShape3D, ~1.0×0.5×1.0, for interaction radius)
├── CSGBox3D (1.0×0.5×1.0, material: brown #8B6914)
├── Label3D ("Wood Pile" above)
├── GdPAIObjectData → WoodPileObject (script)
├── GdPAIInteractable (script)
└── GdPAILocationData (script)
```

#### Campfire — `examples/source_3d/prefabs/campfire_3d.tscn`

```
Node3D (root)
├── CollisionShape3D (CylinderShape3D, radius ~0.8, height ~0.3, for interaction radius)
├── CSGSphere3D (radius 0.6, material: orange-red #E85D3F)       ← fire glow
├── CSGCylinder3D (radius 0.15, height 0.8, brown #5C3A1E)       ← log 1
├── CSGCylinder3D (radius 0.15, height 0.8, rotated, brown)      ← log 2
├── OmniLight3D (warm orange, small range, flicker optional)
├── Label3D ("Campfire (100%)" above, updated by scene script)
├── GdPAIObjectData → CampfireObject (script)
├── GdPAIInteractable (script)
└── GdPAILocationData (script)
```

#### Potato — `examples/source_3d/prefabs/potato_3d.tscn`

```
Node3D (root)
├── CollisionShape3D (SphereShape3D, radius ~0.3, for interaction radius)
├── CSGSphere3D (radius 0.25, material: tan #D2B48C)
├── Label3D ("Potato" above)
├── GdPAIObjectData → PotatoObject (script)
├── GdPAIInteractable (script)
└── GdPAILocationData (script)
```

### Campfire Runtime Scene Script

Both 2D and 3D campfire prefabs need a small runtime script (separate from `CampfireObject`) to:
- Decay `campfire_object.current_fuel` each frame
- Update the Label/Label3D text to show current fuel percentage
- Optionally scale/tint the fire primitive based on fuel level

### Demo Scene Assembly

#### 2D — `examples/campfire_2d.tscn`

```
Node2D (root)
├── NavigationRegion2D (covers play area, e.g. 800×600)
├── Campfire (instance of campfire_2d.tscn, positioned at center)
├── WoodPile × 4 (instances, positioned around the map)
├── PotatoSpawner (node with PotatoSpawner script, spawn_area covering map)
├── Agent × 2–4 (instances of agent_2d.tscn, scattered around)
└── (optional) TileMap or ColorRect background for ground
```

#### 3D — `examples/campfire_3d.tscn`

```
Node3D (root)
├── NavigationRegion3D (with baked NavMesh, covers play area)
├── CSGBox3D (ground plane, large flat box, material: dark green/brown)
├── DirectionalLight3D + Camera3D (overhead or angled view)
├── Campfire (instance of campfire_3d.tscn, positioned at center)
├── WoodPile × 4 (instances, positioned around the map)
├── PotatoSpawner (node with PotatoSpawner script, spawn_area covering map)
├── Agent × 2–4 (instances of agent_3d.tscn, scattered around)
```

---

## Implementation Challenges & Solutions

### Challenge 1: Fire Fuel Tracking

**Problem:** Multiple agents need to know the fire's fuel level for planning.

**Solution:** Fire fuel is stored **directly on the `CampfireObject`**
- `CampfireObject` has `current_fuel` property used by planning actions
- A separate scene node/script handles runtime fuel decay and visuals, updating the object data as needed
- Actions reference the campfire object directly via `campfire_ref`
- `AddFuelAction` and `CookPotatoAction` check `campfire_ref.current_fuel` in `get_action_cost()`

### Challenge 2: Resource Depletion

**Solution:**
- **Wood piles:** Infinite, no depletion. Keeps example simple.
- **Potatoes:** Despawn when picked up (`queue_free` in `DigPotatoAction.perform_action`), respawn at random locations via `PotatoSpawner`

### Challenge 3: Visual Feedback

**Solution:**
- All objects use **Godot primitive nodes** (ColorRect, Circle) — see [Prefab Prototypes](#prefab-prototypes-primitive-shapes) above
- All objects have Label showing their type
- Campfire label shows fuel percentage, updated by a small runtime scene script
- Agent labels show held item via existing `AgentDebugLabel`
- Primitives use distinct colors per object type for at-a-glance readability
- Swapping to real art later is just replacing primitive children — scripts and collision stay intact

### Challenge 4: Eating Ownership

**Solution (already implemented):**
- `EatHeldFoodAction` is provided by `HungerBehaviorConfig`
- Uses `get_requirements()` → `RequirementSpec.binding_exists("held_item")`
- `PickupAction` (and new interaction actions) provide `held_item` binding via `get_provisions()`
- Hunger restoration keyed by held item id in `hunger_restored_by_item` dictionary

---

## Testing Checklist

**Fire Mechanics:**
- [ ] Fire fuel decays at expected rate (3/sec)
- [ ] Agent holding wood can add fuel to fire
- [ ] Agent not holding wood navigates to pile first
- [ ] Can't add fuel if fire is already full (100)

**Cooking System:**
- [ ] Agent can dig up potato (sets held_item = "potato")
- [ ] Agent holding raw potato can cook it at campfire (if fuel >= 20)
- [ ] Cooking transforms held_item from "potato" to "cooked_potato"
- [ ] Can't cook if fire fuel < min_fuel_to_cook threshold
- [ ] Agent can eat cooked potato (reduces hunger by 50)
- [ ] Can't eat if not holding an edible item

**Inventory Management:**
- [ ] Can't pick up wood while already holding something
- [ ] Can't pick up potato while already holding something
- [ ] Agent can drop held items when replanning requires free hands
- [ ] held_item correctly shows: "", "wood", "potato", or "cooked_potato"

**Potato Spawning:**
- [ ] PotatoSpawner spawns initial potatoes at random locations
- [ ] Potatoes despawn when picked up
- [ ] Potatoes respawn at new random locations after delay
- [ ] Never more than max_potatoes active at once

**AI Planning:**
- [ ] Hungry agent prioritizes getting food when hunger reward > fire reward
- [ ] Agent completes full chain: GoTo → dig potato → GoTo → cook → eat
- [ ] Agent recognizes need to refuel fire before cooking
- [ ] Multiple agents can share fire maintenance duty
- [ ] Agent with low hunger + low fire balances both needs

**Integration tests:**
- [ ] `test/integration/test_campfire_smoke.gd` — end-to-end planning smoke test
- [ ] `test/integration/test_campfire_chains.gd` — specific scenario tests

---

## Future Enhancements (Out of Scope for v1)

- **Wood pile cooldown** — piles need time to "regrow" after pickup
- **Fire goes out visual** — if fuel reaches 0, campfire visuals turn off/dim significantly
- **Relight action** — requires holding wood + using tinderbox/flint item to restart dead fire
- **Multiple fires** — agents choose which fire to maintain based on proximity
- **Other cookable foods** — fish, meat, etc. with different cook times
- **Partial cooking** — food burns if left too long
- **Fuel consumption during cooking** — cooking uses some fire fuel, not just blocking threshold

---

## Integration Tests

Tests follow the same pattern as `test/integration/test_hunger_example_smoke.gd`:
instantiate real prefabs, set blackboard state, trigger planning, and verify action chain titles.

### Test file: `test/integration/test_campfire_example_smoke.gd`

```gdscript
extends GutTest

const AGENT_2D_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/agent_2d.tscn")
const WOOD_PILE_2D_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/wood_pile_2d.tscn")
const CAMPFIRE_2D_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/campfire_2d.tscn")
const POTATO_2D_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/potato_2d.tscn")
const WORLD_NODE_SCRIPT: Script = preload(
    "res://addons/GdPlanningAI/scripts/nodes/gdpai_world_node.gd"
)
const BLACKBOARD_PLAN_SCRIPT: Script = preload(
    "res://addons/GdPlanningAI/scripts/gdpai_blackboard_plan.gd"
)


func after_each() -> void:
    for node in get_tree().get_nodes_in_group("GdPAIObjectData"):
        if is_instance_valid(node.entity):
            node.entity.queue_free()
    await _drain_scheduler()


func _drain_scheduler(timeout_frames: int = 120) -> void:
    var scheduler: GdPAIPlanScheduler = _scheduler()
    if scheduler == null:
        return
    for i in range(timeout_frames):
        scheduler.process_callbacks()
        if scheduler.active_job_count() == 0:
            return
        await get_tree().process_frame


func _make_world_node() -> GdPAIWorldNode:
    var world_node: GdPAIWorldNode = WORLD_NODE_SCRIPT.new()
    world_node.name = "GdPAIWorldNode"
    world_node.blackboard_plan = BLACKBOARD_PLAN_SCRIPT.new()
    add_child_autofree(world_node)
    return world_node


func _pump_frames(frames: int) -> void:
    for i in range(frames):
        await get_tree().physics_frame
        await get_tree().process_frame


func _scheduler() -> GdPAIPlanScheduler:
    return GdPAIAutoload.get_scheduler()


func _start_plan_and_wait(agent: GdPAIAgent, timeout_frames: int = 300) -> Array[Action]:
    var scheduler: GdPAIPlanScheduler = _scheduler()
    var previous_plan: Array[Action] = agent.get_current_plan()
    agent.manually_start_plan()
    var saw_job: bool = scheduler.active_job_count() > 0
    for i in range(timeout_frames):
        scheduler.process_callbacks()
        saw_job = saw_job or scheduler.active_job_count() > 0
        if saw_job and scheduler.active_job_count() == 0:
            return agent.get_current_plan()
        if agent.get_current_plan() != previous_plan:
            return agent.get_current_plan()
        await get_tree().process_frame
    fail_test("Timed out waiting for submitted agent plan")
    return []


func _plan_titles(plan: Array[Action]) -> String:
    var titles: Array[String] = []
    for action in plan:
        titles.append(action.get_title())
    return " -> ".join(titles)


func _setup_campfire_scene() -> Dictionary:
    """Create world node, campfire, wood piles, and potatoes. Returns {agent, campfire}."""
    _make_world_node()

    # Campfire at center
    var campfire_entity: Node2D = CAMPFIRE_2D_PREFAB.instantiate()
    add_child_autofree(campfire_entity)
    campfire_entity.global_position = Vector2(400, 300)
    var campfire: CampfireObject = GdPAIUTILS.get_child_of_type(campfire_entity, CampfireObject)

    # Wood piles around the map
    var wood_positions: Array[Vector2] = [
        Vector2(200, 200), Vector2(600, 200),
        Vector2(200, 400), Vector2(600, 400),
    ]
    for pos in wood_positions:
        var wood_entity: Node2D = WOOD_PILE_2D_PREFAB.instantiate()
        add_child_autofree(wood_entity)
        wood_entity.global_position = pos

    # Potatoes scattered around
    var potato_positions: Array[Vector2] = [
        Vector2(300, 150), Vector2(500, 150),
        Vector2(150, 350), Vector2(650, 350),
        Vector2(400, 450),
    ]
    for pos in potato_positions:
        var potato_entity: Node2D = POTATO_2D_PREFAB.instantiate()
        add_child_autofree(potato_entity)
        potato_entity.global_position = pos

    # Agent
    var agent_entity: Node2D = AGENT_2D_PREFAB.instantiate()
    add_child_autofree(agent_entity)
    agent_entity.global_position = Vector2(100, 100)
    var agent: GdPAIAgent = GdPAIUTILS.get_child_of_type(agent_entity, GdPAIAgent)
    agent.config.planning_strategy = GdPAIAgentConfig.PlanningStrategy.ON_DEMAND

    return {"agent": agent, "campfire": campfire}


# ── Scenario Tests ─────────────────────────────────────────────


func test_full_cooking_chain() -> void:
    """Hunger=70, Fire=80, empty hands → GoTo potato → Dig → GoTo campfire → Cook → Eat"""
    var setup: Dictionary = _setup_campfire_scene()
    var agent: GdPAIAgent = setup["agent"]
    var campfire: CampfireObject = setup["campfire"]

    campfire.current_fuel = 80.0
    agent.blackboard.set_property("hunger", 70.0)
    agent.blackboard.set_property("held_item", "")
    await _pump_frames(3)

    var plan: Array[Action] = await _start_plan_and_wait(agent)
    assert_false(plan.is_empty(), "Agent should plan when hungry with fire available")

    # Expected: GoTo(potato) → Dig Potato → GoTo(campfire) → Cook Potato → Eat Held Food
    assert_eq(plan.size(), 5, "Plan should have 5 actions: %s" % _plan_titles(plan))
    assert_eq(plan[0].get_title(), "Go To")
    assert_eq(plan[1].get_title(), "Dig Potato")
    assert_eq(plan[2].get_title(), "Go To")
    assert_eq(plan[3].get_title(), "Cook Potato")
    assert_eq(plan[4].get_title(), "Eat Held Food")


func test_fire_too_low_to_cook() -> void:
    """Hunger=60, Fire=10, holding potato → Drop → refuel → re-dig → cook → eat"""
    var setup: Dictionary = _setup_campfire_scene()
    var agent: GdPAIAgent = setup["agent"]
    var campfire: CampfireObject = setup["campfire"]

    campfire.current_fuel = 10.0
    agent.blackboard.set_property("hunger", 60.0)
    agent.blackboard.set_property("held_item", "potato")
    await _pump_frames(3)

    var plan: Array[Action] = await _start_plan_and_wait(agent)
    assert_false(plan.is_empty(), "Agent should plan when fire is too low to cook")

    # Expected: Drop → GoTo(wood) → Pick Up Wood → GoTo(campfire) → Add Fuel
    #           → GoTo(potato) → Dig Potato → GoTo(campfire) → Cook Potato → Eat
    assert_eq(plan[0].get_title(), "Drop Item",
        "First action should be Drop, got: %s" % _plan_titles(plan))
    assert_eq(plan[1].get_title(), "Go To")
    assert_eq(plan[2].get_title(), "Pick Up Wood")
    assert_eq(plan[3].get_title(), "Go To")
    assert_eq(plan[4].get_title(), "Add Fuel")
    # After refueling, the chain continues with dig → cook → eat
    var titles: Array[String] = []
    for a in plan:
        titles.append(a.get_title())
    assert_true(titles.has("Dig Potato"), "Plan should include Dig Potato after refueling")
    assert_true(titles.has("Cook Potato"), "Plan should include Cook Potato after refueling")
    assert_true(titles.has("Eat Held Food"), "Plan should include Eat Held Food")


func test_preemptive_fire_maintenance() -> void:
    """Hunger=20, Fire=35, empty hands → wood first, then food"""
    var setup: Dictionary = _setup_campfire_scene()
    var agent: GdPAIAgent = setup["agent"]
    var campfire: CampfireObject = setup["campfire"]

    campfire.current_fuel = 35.0
    agent.blackboard.set_property("hunger", 20.0)
    agent.blackboard.set_property("held_item", "")
    await _pump_frames(3)

    var plan: Array[Action] = await _start_plan_and_wait(agent)
    assert_false(plan.is_empty(), "Agent should plan when fire is moderate and hunger is low")

    # Fire reward (40) > hunger (20), so fire maintenance should come first
    # Expected: GoTo(wood) → Pick Up Wood → GoTo(campfire) → Add Fuel
    #           → GoTo(potato) → Dig Potato → GoTo(campfire) → Cook Potato → Eat
    var titles: Array[String] = []
    for a in plan:
        titles.append(a.get_title())
    var wood_idx: int = titles.find("Pick Up Wood")
    var dig_idx: int = titles.find("Dig Potato")
    assert_true(wood_idx >= 0, "Plan should include Pick Up Wood")
    assert_true(dig_idx >= 0, "Plan should include Dig Potato")
    assert_true(wood_idx < dig_idx,
        "Wood gathering should come before potato digging when fire reward > hunger")


func test_competing_priorities_hunger_wins() -> void:
    """Hunger=95, Fire=15, empty hands → hunger (95) > fire reward (40) → food first"""
    var setup: Dictionary = _setup_campfire_scene()
    var agent: GdPAIAgent = setup["agent"]
    var campfire: CampfireObject = setup["campfire"]

    campfire.current_fuel = 15.0
    agent.blackboard.set_property("hunger", 95.0)
    agent.blackboard.set_property("held_item", "")
    await _pump_frames(3)

    var plan: Array[Action] = await _start_plan_and_wait(agent)
    assert_false(plan.is_empty(), "Agent should plan when both hunger and fire are critical")

    # Hunger reward (95) > fire reward (40), so food should come first
    var titles: Array[String] = []
    for a in plan:
        titles.append(a.get_title())
    var dig_idx: int = titles.find("Dig Potato")
    var wood_idx: int = titles.find("Pick Up Wood")
    assert_true(dig_idx >= 0, "Plan should include Dig Potato")
    assert_true(wood_idx >= 0, "Plan should include Pick Up Wood")
    assert_true(dig_idx < wood_idx,
        "Food gathering should come before wood when hunger > fire reward")


func test_cannot_add_fuel_when_full() -> void:
    """Fire=100, holding wood → AddFuel should be invalid (cost=INF)"""
    var setup: Dictionary = _setup_campfire_scene()
    var agent: GdPAIAgent = setup["agent"]
    var campfire: CampfireObject = setup["campfire"]

    campfire.current_fuel = 100.0
    agent.blackboard.set_property("hunger", 10.0)
    agent.blackboard.set_property("held_item", "wood")
    await _pump_frames(3)

    var plan: Array[Action] = await _start_plan_and_wait(agent)
    # Agent should not plan AddFuel when fire is full
    # It should either wander or drop wood and do something else
    var titles: Array[String] = []
    for a in plan:
        titles.append(a.get_title())
    assert_false(titles.has("Add Fuel"),
        "Should not plan Add Fuel when fire is full, got: %s" % _plan_titles(plan))


func test_cannot_cook_without_potato() -> void:
    """Fire=80, holding wood → CookPotato should be invalid (wrong held_item)"""
    var setup: Dictionary = _setup_campfire_scene()
    var agent: GdPAIAgent = setup["agent"]
    var campfire: CampfireObject = setup["campfire"]

    campfire.current_fuel = 80.0
    agent.blackboard.set_property("hunger", 70.0)
    agent.blackboard.set_property("held_item", "wood")
    await _pump_frames(3)

    var plan: Array[Action] = await _start_plan_and_wait(agent)
    var titles: Array[String] = []
    for a in plan:
        titles.append(a.get_title())
    # CookPotato requires held_item="potato", so it should not appear when holding wood
    # But AddFuel should be valid
    assert_false(titles.has("Cook Potato"),
        "Should not plan Cook Potato when holding wood, got: %s" % _plan_titles(plan))
```

### What Each Test Validates

| Test | Validates |
|---|---|
| `test_full_cooking_chain` | GoTo→Dig→GoTo→Cook→Eat chain when fire is adequate |
| `test_fire_too_low_to_cook` | Drop→refuel→re-dig chain when fire blocks cooking |
| `test_preemptive_fire_maintenance` | Fire maintenance ordered before food when fire reward > hunger |
| `test_competing_priorities_hunger_wins` | Food ordered before fire when hunger reward > fire reward |
| `test_cannot_add_fuel_when_full` | `AddFuelAction.get_action_cost()` returns INF when fire=100 |
| `test_cannot_cook_without_potato` | `CookPotatoAction` precondition blocks when not holding potato |

### Running

```bash
# Build Rust binary first, then:
make test-godot
# Or run specific test:
godot --headless --path . -s addons/gut/gut_cmdln.gd -gtest=test/integration/test_campfire_example_smoke.gd
```

---

## Estimated Implementation Time

**New gameplay code:**
- WoodPileObject + PickUpWoodAction: 0.5 hours
- CampfireObject + AddFuelAction + CookPotatoAction: 1.5 hours
- PotatoObject + DigPotatoAction: 0.5 hours
- PotatoSpawner: 0.5 hours
- CampfireBehaviorConfig update (add GoToAction): 5 minutes

**2D + 3D Demo:**
- 2D prefabs (4×) with primitive shapes + labels: 0.75 hours
- 3D prefabs (4×) with CSG primitives + Label3D: 0.75 hours
- Campfire runtime scene script (shared, fuel decay + label update): 0.5 hours
- 2D demo scene assembly + PotatoSpawner setup: 0.5 hours
- 3D demo scene assembly + NavMesh bake: 0.75 hours

**Integration tests:** 1.5 hours (test file + debug against implementation)
**Testing + bug fixes:** 1-1.5 hours

**Total:** ~9-10 hours
