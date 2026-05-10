# GdPlanningAI — Campfire Tending Example

> **Last updated:** 2026-05-10 — rewritten for GoToAction architecture.
> Existing components (`CampfireBehaviorConfig`, `MaintainFireGoal`, `EatHeldFoodAction`,
> `DropItemAction`, `PickupAction`, `HoldableObject`, `HungerGoal`, `HungerBehaviorConfig`,
> `HungerPropertyUpdater`) are already implemented and are referenced rather than re-specified.

---

## Agreed v1 Implementation Scope

- **Primary target:** Implement the **2D version first**. Shared gameplay logic should be written so it can be reused by a later 3D pass, but 3D scene/prefab work is deferred.
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

**Note for v1:** The first implementation uses a **parameterized fixed reward** for fire maintenance and focuses on proving the core planning loop in 2D.

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

**New 2D prefabs:**
- `examples/source_2d/prefabs/wood_pile_2d.tscn`
- `examples/source_2d/prefabs/campfire_2d.tscn`
- `examples/source_2d/prefabs/potato_2d.tscn`
- `examples/source_2d/prefabs/agent_2d.tscn`

**Deferred to a later pass:**
- `examples/campfire_3d.tscn`
- 3D prefabs for all objects + agents

---

## Prefab Prototypes (Primitive Shapes)

All prototype prefabs use **Godot primitive nodes only** — no imported assets, no custom textures.
This makes it trivial to swap in real art later by replacing the primitive children while keeping
the script, collision, and label structure intact.

### Agent — `examples/source_2d/prefabs/agent_2d.tscn`

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

### Wood Pile — `examples/source_2d/prefabs/wood_pile_2d.tscn`

```
Node2D (root)
├── CollisionShape2D (RectangleShape2D, ~32×16, for interaction radius)
├── ColorRect (32×16, brown #8B6914, centered)
├── Label ("Wood Pile" above)
├── GdPAIObjectData → WoodPileObject (script)
├── GdPAIInteractable (script)
└── GdPAILocationData (script)
```

### Campfire — `examples/source_2d/prefabs/campfire_2d.tscn`

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

The campfire scene also needs a small runtime script (separate from `CampfireObject`) to:
- Decay `campfire_object.current_fuel` each frame
- Update the Label text to show current fuel percentage
- Optionally scale/tint the fire Circle based on fuel level

### Potato — `examples/source_2d/prefabs/potato_2d.tscn`

```
Node2D (root)
├── CollisionShape2D (CircleShape2D, radius ~10, for interaction radius)
├── Circle (radius 8, color: tan #D2B48C, centered)
├── Label ("Potato" above)
├── GdPAIObjectData → PotatoObject (script)
├── GdPAIInteractable (script)
└── GdPAILocationData (script)
```

### Demo Scene Assembly — `examples/campfire_2d.tscn`

```
Node2D (root)
├── NavigationRegion2D (covers play area, e.g. 800×600)
├── Campfire (instance of campfire_2d.tscn, positioned at center)
├── WoodPile × 4 (instances, positioned around the map)
├── PotatoSpawner (node with PotatoSpawner script, spawn_area covering map)
├── Agent × 2–4 (instances of agent_2d.tscn, scattered around)
└── (optional) TileMap or ColorRect background for ground
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

## Estimated Implementation Time

**New gameplay code:**
- WoodPileObject + PickUpWoodAction: 0.5 hours
- CampfireObject + AddFuelAction + CookPotatoAction: 1.5 hours
- PotatoObject + DigPotatoAction: 0.5 hours
- PotatoSpawner: 0.5 hours
- CampfireBehaviorConfig update (add GoToAction): 5 minutes

**2D Demo:**
- Prefabs (campfire, wood, potato, agent) with primitive shapes + labels: 1 hour
- Campfire runtime scene script (fuel decay + label update): 0.5 hours
- Demo scene assembly + PotatoSpawner setup: 0.5 hours
- Navigation setup: 0.5 hours

**Integration tests:** 1 hour
**Testing + bug fixes:** 1-1.5 hours

**Total:** ~7-8 hours
