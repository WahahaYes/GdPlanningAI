# GdPlanningAI — Campfire Tending Example

## Prerequisites

**This example assumes examples have been moved to the top-level `examples/` folder.** See `EXAMPLES_REORGANIZATION.md` for migration steps if not yet completed.

---

## Agreed v1 Implementation Scope

- **Primary target:** Implement the **2D version first**. Shared gameplay logic should be written so it can be reused by a later 3D pass, but 3D scene/prefab work is deferred.
- **Fire maintenance reward:** `MaintainFireGoal` should use a **simple exported/parameterized reward value** in v1 rather than computing a dynamic reward from fire fuel.
- **Fire fuel access:** Campfire-related actions may **query the referenced campfire object directly** for runtime/planning validity and cost checks in the first pass.
- **Inventory flexibility:** Add a **minimal `DropItemAction`** so agents can recover from plans where they must switch from holding food to holding wood.
- **Eating action ownership:** Resource objects should provide pickup/interaction actions, but eating should be an **agent-provided action**. `EatHeldFoodAction` now lives on the shared hunger behavior and restores hunger from a configurable per-item dictionary keyed by held item id.
- **Object data responsibility:** `GdPAIObjectData` should remain focused on **planning-system integration only**. Any campfire fuel visuals, labels, or scene presentation should live in separate scene nodes/scripts.
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

**Note for v1:** Although dynamic/threshold-based fire maintenance is an interesting extension, the first implementation uses a **parameterized fixed reward** for fire maintenance and focuses on proving the core planning loop in 2D.

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

## Reused Infrastructure

This example leverages existing systems with minimal new code:

| Existing System | How We Use It |
|---|---|
| `PropertyUpdater` | `HungerUpdater` decays hunger over time |
| `SpatialAction` | Navigate to wood piles, potato spawns, campfire |
| `GdPAIObjectData` | `WoodPileObject`, `PotatoObject`, `CampfireObject` provide actions |
| Property decay pattern | Hunger increases over time (like existing systems) |
| Timed interactions | `CookPotatoAction`, `AddFuelAction` have brief durations |
| Dynamic validity | Can't cook without holding potato AND fire fuel > threshold |
| Multi-step chains | Dig potato → cook potato → eat cooked potato |
| Object-attached properties | Fire object tracks its own fuel level |

**Net new systems:** Potato respawn manager (randomized location spawning), minimal drop action, and an agent-provided eat-held-food action. Otherwise just new goal/action/object implementations.

---

## New Components

### Behaviors

#### `shared/behaviors/fire_maintenance/`

**`fire_maintenance_goal.gd`** (`MaintainFireGoal extends Goal`)
```gdscript
@export var reward_value: float = 40.0

# v1: fixed parameterized reward so it can compete with HungerGoal
func compute_reward(agent: GdPAIAgent) -> float:
    return reward_value

func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
    # Goal is satisfied when we've added fuel to the fire
    # The AddFuelAction will increase the fire's fuel property
    return []
```

**`hunger_goal.gd`** (`HungerGoal extends Goal`)
```gdscript
# Reward scales from 0 (full) to 100 (starving)
func compute_reward(agent: GdPAIAgent) -> float:
    var hunger: float = agent.blackboard.get_property("hunger")
    if hunger == null:
        return 0.0
    return max(0.0, hunger)

func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
    var current_hunger: float = agent.blackboard.get_property("hunger")
    # Want hunger to decrease
    return [Precondition.agent_property_less_than("hunger", current_hunger)]
```

**`hunger_updater.gd`** (`HungerUpdater extends PropertyUpdater`)
```gdscript
var hunger_rate: float = 2.0  # Hunger per second
var initial_hunger: float = 50.0

func initialize(agent: GdPAIAgent) -> void:
    agent.blackboard.set_property("hunger", initial_hunger)
    if not agent.blackboard.has_property("held_item"):
        agent.blackboard.set_property("held_item", "")  # "", "wood", "potato", "cooked_potato"

func update_properties(agent: GdPAIAgent, delta: float) -> void:
    var hunger: float = agent.blackboard.get_property("hunger")
    agent.blackboard.set_property("hunger", min(100.0, hunger + hunger_rate * delta))
```

**`hunger_behavior_config.gd`** (`HungerBehaviorConfig extends GdPAIBehaviorConfig`)
```gdscript
@export var hunger_restored_by_item: Dictionary = {
    "banana": 20.0,
    "cooked_potato": 50.0,
}
@export var eat_duration: float = 1.5

func _populate(
    goals: Array[Goal],
    actions: Array[Action],
    updaters: Array[PropertyUpdater]
) -> void:
    goals.append(HungerGoal.new())
    actions.append(EatHeldFoodAction.new(hunger_restored_by_item, eat_duration))
    updaters.append(HungerUpdater.new())
```

**`campfire_behavior_config.gd`** (`CampfireBehaviorConfig extends GdPAIBehaviorConfig`)
```gdscript
func _populate(
    goals: Array[Goal],
    actions: Array[Action],
    updaters: Array[PropertyUpdater]
) -> void:
    goals.append(MaintainFireGoal.new())
    actions.append(DropItemAction.new())
```

Agents in the campfire example should be configured with both `HungerBehaviorConfig`
and `CampfireBehaviorConfig`. Hunger owns `held_item`, `EatHeldFoodAction`, and hunger
restoration values; campfire owns fire-maintenance logic and task-switching via drop.

---

### Objects

#### `shared/objects/holdable/`

**`holdable_object.gd`** (`HoldableObject extends GdPAIObjectData`)
```gdscript
@export var item_id: String = ""
@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData

func get_group_labels() -> Array[String]:
    return ["HoldableObject", "GdPAIObjectData"]

func get_provided_actions() -> Array[Action]:
    return [PickupAction.new(location_data, interactable_attribs, self)]

func get_sim_properties() -> Dictionary:
    return {
        "item_id": item_id,
    }
```

**`pickup_action.gd`** (`PickupAction extends SpatialAction`)
```gdscript
func get_validity_checks() -> Array[Precondition]:
    var checks: Array[Precondition] = super()
    checks.append(Precondition.agent_has_property("held_item"))
    checks.append(Precondition.check_is_object_valid(holdable_item))
    return checks

func get_preconditions() -> Array[Precondition]:
    return [Precondition.agent_property_equal_to("held_item", "")]

func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> void:
    super(agent_blackboard, world_state)
    agent_blackboard.set_property("held_item", holdable_item.item_id)
```

#### `shared/objects/wood_pile/`

**`wood_pile_object.gd`** (`WoodPileObject extends GdPAIObjectData`)
```gdscript
@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData
@export var label_text: String = "Wood Pile"  # Displayed in scene

func get_group_labels() -> Array[String]:
    return ["WoodPileObject", "GdPAIObjectData"]

func get_provided_actions() -> Array[Action]:
    return [PickUpWoodAction.new(location_data, interactable_attribs)]

func get_sim_properties() -> Dictionary:
    return {}
```

**`pick_up_wood_action.gd`** (`PickUpWoodAction extends SpatialAction`)
```gdscript
# Navigate to wood pile and pick up wood
func get_validity_checks() -> Array[Precondition]:
    var checks: Array[Precondition] = super()
    checks.append(Precondition.agent_has_property("held_item"))
    # Can't pick up if already holding something
    checks.append(Precondition.agent_property_equal_to("held_item", ""))
    return checks

func get_action_cost(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> float:
    var cost: float = super(agent_blackboard, world_state)
    if cost == INF:
        return INF
    return cost + 0.5  # Brief pickup duration

func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> void:
    super(agent_blackboard, world_state)
    agent_blackboard.set_property("held_item", "wood")

func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
    var parent_status: Action.Status = super(agent, delta)
    if parent_status == Action.Status.FAILURE:
        return Action.Status.FAILURE
    
    if not get_state(agent, "target_reached"):
        return Action.Status.RUNNING
    
    # Instant pickup once at location
    agent.blackboard.set_property("held_item", "wood")
    return Action.Status.SUCCESS
```

---

#### `shared/objects/campfire/`

**`campfire_object.gd`** (`CampfireObject extends GdPAIObjectData`)
```gdscript
@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData
@export var fuel_per_wood: float = 30.0  # How much fuel one wood restores
@export var min_fuel_to_cook: float = 20.0  # Minimum fuel required for cooking

# Planning-relevant campfire state only
var current_fuel: float = 100.0
var fuel_decay_rate: float = 3.0  # Fuel per second

func get_group_labels() -> Array[String]:
    return ["CampfireObject", "GdPAIObjectData"]

func get_provided_actions() -> Array[Action]:
    return [
        AddFuelAction.new(self, location_data, interactable_attribs, fuel_per_wood),
        CookPotatoAction.new(self, location_data, interactable_attribs, min_fuel_to_cook)
    ]

func get_sim_properties() -> Dictionary:
    return {
        "fuel_per_wood": fuel_per_wood,
        "current_fuel": current_fuel,
        "min_fuel_to_cook": min_fuel_to_cook
    }
```

**`add_fuel_action.gd`** (`AddFuelAction extends SpatialAction`)
```gdscript
const ADD_FUEL_DURATION: float = 1.0
var campfire_ref: CampfireObject
var fuel_per_wood: float

func _init(
    p_campfire: CampfireObject,
    p_object_location: GdPAILocationData,
    p_interactable_attribs: GdPAIInteractable,
    p_fuel_per_wood: float
) -> void:
    super(p_object_location, p_interactable_attribs)
    campfire_ref = p_campfire
    fuel_per_wood = p_fuel_per_wood

func get_validity_checks() -> Array[Precondition]:
    var checks: Array[Precondition] = super()
    checks.append(Precondition.agent_has_property("held_item"))
    # Must be holding wood to add it
    checks.append(Precondition.agent_property_equal_to("held_item", "wood"))
    return checks

func get_action_cost(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> float:
    var cost: float = super(agent_blackboard, world_state)
    if cost == INF:
        return INF
    # Don't add fuel if fire is already full
    if campfire_ref.current_fuel >= 100.0:
        return INF
    return cost + ADD_FUEL_DURATION

func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> void:
    super(agent_blackboard, world_state)
    # Simulate adding fuel to fire
    agent_blackboard.set_property("held_item", "")

func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
    if super(agent) == Action.Status.FAILURE:
        return Action.Status.FAILURE
    set_state(agent, "add_fuel_elapsed", 0.0)
    return Action.Status.SUCCESS

func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
    var parent_status: Action.Status = super(agent, delta)
    if parent_status == Action.Status.FAILURE:
        return Action.Status.FAILURE
    
    if not get_state(agent, "target_reached"):
        return Action.Status.RUNNING
    
    var elapsed: float = get_state(agent, "add_fuel_elapsed") + delta
    set_state(agent, "add_fuel_elapsed", elapsed)
    
    if elapsed >= ADD_FUEL_DURATION:
        # Actually add fuel to the campfire object
        campfire_ref.current_fuel = min(100.0, campfire_ref.current_fuel + fuel_per_wood)
        agent.blackboard.set_property("held_item", "")
        return Action.Status.SUCCESS
    
    return Action.Status.RUNNING

func post_perform_action(agent: GdPAIAgent) -> Action.Status:
    super(agent)
    erase_state(agent, "add_fuel_elapsed")
    return Action.Status.SUCCESS
```

**`cook_potato_action.gd`** (`CookPotatoAction extends SpatialAction`)
```gdscript
const COOK_DURATION: float = 2.0
var campfire_ref: CampfireObject
var min_fuel_to_cook: float

func _init(
    p_campfire: CampfireObject,
    p_object_location: GdPAILocationData,
    p_interactable_attribs: GdPAIInteractable,
    p_min_fuel_to_cook: float
) -> void:
    super(p_object_location, p_interactable_attribs)
    campfire_ref = p_campfire
    min_fuel_to_cook = p_min_fuel_to_cook

func get_validity_checks() -> Array[Precondition]:
    var checks: Array[Precondition] = super()
    checks.append(Precondition.agent_has_property("held_item"))
    # Must be holding raw potato
    checks.append(Precondition.agent_property_equal_to("held_item", "potato"))
    return checks

func get_action_cost(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> float:
    var cost: float = super(agent_blackboard, world_state)
    if cost == INF:
        return INF
    # Can't cook if fire fuel is too low
    if campfire_ref.current_fuel < min_fuel_to_cook:
        return INF
    return cost + COOK_DURATION

func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> void:
    super(agent_blackboard, world_state)
    # Transform potato to cooked_potato
    agent_blackboard.set_property("held_item", "cooked_potato")

func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
    if super(agent) == Action.Status.FAILURE:
        return Action.Status.FAILURE
    set_state(agent, "cook_elapsed", 0.0)
    return Action.Status.SUCCESS

func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
    var parent_status: Action.Status = super(agent, delta)
    if parent_status == Action.Status.FAILURE:
        return Action.Status.FAILURE
    
    if not get_state(agent, "target_reached"):
        return Action.Status.RUNNING
    
    # Check if fire still has enough fuel
    if campfire_ref.current_fuel < min_fuel_to_cook:
        return Action.Status.FAILURE
    
    var elapsed: float = get_state(agent, "cook_elapsed") + delta
    set_state(agent, "cook_elapsed", elapsed)
    
    if elapsed >= COOK_DURATION:
        agent.blackboard.set_property("held_item", "cooked_potato")
        return Action.Status.SUCCESS
    
    return Action.Status.RUNNING

func post_perform_action(agent: GdPAIAgent) -> Action.Status:
    super(agent)
    erase_state(agent, "cook_elapsed")
    return Action.Status.SUCCESS
```

---

#### `shared/objects/potato/`

**`potato_object.gd`** (`PotatoObject extends GdPAIObjectData`)
```gdscript
@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData
@export var label_text: String = "Potato"  # Displayed in scene

func get_group_labels() -> Array[String]:
    return ["PotatoObject", "GdPAIObjectData"]

func get_provided_actions() -> Array[Action]:
    return [DigPotatoAction.new(location_data, interactable_attribs)]

func get_sim_properties() -> Dictionary:
    return {}
```

**`dig_potato_action.gd`** (`DigPotatoAction extends SpatialAction`)
```gdscript
# Navigate to potato and dig it up
func get_validity_checks() -> Array[Precondition]:
    var checks: Array[Precondition] = super()
    checks.append(Precondition.agent_has_property("held_item"))
    # Can't pick up if already holding something
    checks.append(Precondition.agent_property_equal_to("held_item", ""))
    return checks

func get_action_cost(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> float:
    var cost: float = super(agent_blackboard, world_state)
    if cost == INF:
        return INF
    return cost + 0.7  # Slightly longer than wood pickup

func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> void:
    super(agent_blackboard, world_state)
    agent_blackboard.set_property("held_item", "potato")

func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
    var parent_status: Action.Status = super(agent, delta)
    if parent_status == Action.Status.FAILURE:
        return Action.Status.FAILURE
    
    if not get_state(agent, "target_reached"):
        return Action.Status.RUNNING
    
    # Instant pickup once at location
    agent.blackboard.set_property("held_item", "potato")
    # Trigger potato pickup/despawn on the owning potato object/node
    return Action.Status.SUCCESS
```

**`drop_item_action.gd`** (`DropItemAction extends Action`, now under `shared/objects/holdable/`)
```gdscript
const DROP_DURATION: float = 0.2

func get_validity_checks() -> Array[Precondition]:
    var checks: Array[Precondition] = []
    checks.append(Precondition.agent_has_property("held_item"))
    return checks

func get_preconditions() -> Array[Precondition]:
    return [Precondition.agent_property_not_equal_to("held_item", "")]

func get_action_cost(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> float:
    return DROP_DURATION

func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> void:
    agent_blackboard.set_property("held_item", "")
```

**`eat_held_food_action.gd`** (`EatHeldFoodAction extends Action`)
```gdscript
@export var hunger_restored_by_item: Dictionary = {
    "banana": 20.0,
    "cooked_potato": 50.0,
}

# Configured on the agent behavior side; any held item id in the dictionary is edible.
```

---

#### `shared/systems/potato_spawner/`

**`potato_spawner.gd`** (`PotatoSpawner extends Node`)
```gdscript
@export var potato_scene: PackedScene  # Prefab for potato object
@export var spawn_area: Rect2  # 2D spawn area (or use AABB for 3D)
@export var max_potatoes: int = 5
@export var respawn_time: float = 10.0  # Seconds between respawns

var active_potatoes: Array[Node] = []
var respawn_timer: float = 0.0

func _ready() -> void:
    # Spawn initial potatoes
    for i in range(max_potatoes):
        spawn_potato()

func _process(delta: float) -> void:
    # Clean up despawned potatoes
    active_potatoes = active_potatoes.filter(func(p): return is_instance_valid(p))
    
    # Respawn if below max
    respawn_timer += delta
    if active_potatoes.size() < max_potatoes and respawn_timer >= respawn_time:
        spawn_potato()
        respawn_timer = 0.0

func spawn_potato() -> void:
    if not potato_scene:
        return
    
    var potato: Node = potato_scene.instantiate()
    var random_pos: Vector2 = Vector2(
        randf_range(spawn_area.position.x, spawn_area.position.x + spawn_area.size.x),
        randf_range(spawn_area.position.y, spawn_area.position.y + spawn_area.size.y)
    )
    
    # Set position (adjust for 2D vs 3D)
    if potato is Node2D:
        potato.position = random_pos
    elif potato is Node3D:
        potato.position = Vector3(random_pos.x, 0, random_pos.y)
    
    get_parent().add_child(potato)
    active_potatoes.append(potato)
    
    # Connect signal to remove when picked up
    if potato.has_signal("picked_up"):
        potato.picked_up.connect(_on_potato_picked_up.bind(potato))

func _on_potato_picked_up(potato: Node) -> void:
    active_potatoes.erase(potato)
    potato.queue_free()
```

---

## Demo Scene Setup

### 2D Version: `examples/campfire_2d.tscn`

**World Layout:**
- Central campfire (visual: simple colored polygon/circle + Label showing fuel %)
- 3-4 wood pile locations (simple sprites + Labels saying "Wood Pile")
- PotatoSpawner node managing 5 randomized potato spawns
- Potato objects (simple sprites + Labels saying "Potato")
- NavigationRegion2D covering playable area

**Agent Setup:**
- 2-4 agents (CharacterBody2D + NavigationAgent2D + Labels showing held_item)
- `HungerBehaviorConfig` + `CampfireBehaviorConfig`
- Each agent starts with `hunger = 50`, `held_item = ""`

### 3D Version: `examples/campfire_3d.tscn`

**World Layout:**
- Central campfire (CSGCylinder for logs + OmniLight3D + Label3D showing fuel %)
- 3-4 wood pile locations (CSGCylinder stacks + Label3D "Wood Pile")
- PotatoSpawner node managing 5 randomized potato spawns
- Potato objects (CSGSphere primitives + Label3D "Potato")
- NavigationRegion3D with baked NavMesh
- Simple plane for ground (CSGBox or PlaneMesh)

**Agent Setup:**
- 2-4 agents (CharacterBody3D + NavigationAgent3D + Label3D showing held_item)
- `HungerBehaviorConfig` + `CampfireBehaviorConfig`
- Each agent starts with `hunger = 50`, `held_item = ""`

**Visual Labels:**
All objects and agents display Label3D (or Label for 2D) showing their state/type

---

## Implementation Challenges & Solutions

### Challenge 1: Fire Fuel Tracking

**Problem:** Multiple agents need to know the fire's fuel level for planning.

**Solution:** Fire fuel is stored **directly on the CampfireObject**
- `CampfireObject` has `current_fuel` property used by planning actions
- A separate scene node/script should handle runtime fuel decay and visuals, updating the object data as needed
- Actions reference the campfire object directly via `campfire_ref`
- `AddFuelAction` and `CookPotatoAction` check `campfire_ref.current_fuel` in `get_action_cost()`
- No world state or agent property mirroring needed - just direct object reference

### Challenge 2: Resource Depletion

**Problem:** Should wood piles and potatoes deplete when harvested?

**Solution:**
- **Wood piles:** Infinite, no depletion. Keeps example simple.
- **Potatoes:** Despawn when picked up, respawn at random locations via `PotatoSpawner`
- This demonstrates both static resources (wood) and dynamic respawning (potatoes)

### Challenge 3: Visual Feedback

**Problem:** How to show object states and agent inventory?

**Solution:**
- All objects have Label/Label3D showing their type ("Campfire", "Wood Pile", "Potato")
- Campfire label shows fuel percentage: "Campfire (67%)"
- Agent labels show held item: "Agent (holding: wood)" or "Agent (holding: —)"
- Simple primitives with clear text labels prioritize clarity over visuals

### Challenge 4: Potato Pickup Signal

**Problem:** How does PotatoSpawner know when a potato is picked up?

**Solution:** 
- `DigPotatoAction` should trigger pickup on the potato object/node when the action succeeds
- The potato scene can then emit a `picked_up` signal or otherwise notify the spawner
- Keep the action responsible for initiating pickup; keep the spawner responsible for tracking active potato instances

### Challenge 5: Eating Ownership

**Problem:** Eating is not tied to a world object, so where should that action live?

**Solution:**
- Pickup and world interactions stay on the relevant objects (`WoodPileObject`, `PotatoObject`, `CampfireObject`)
- Shared pickup/drop mechanics live under the reusable holdable contract (`HoldableObject`, `PickupAction`, `DropItemAction`)
- Eating is provided directly by `HungerBehaviorConfig` as `EatHeldFoodAction`
- Hunger restoration is keyed by held item id so bananas and cooked potatoes can share the same self-eating action

---

## Planning Scenarios Demonstrated

### Scenario 1: Full Cooking Chain
**State:** Hunger = 70, Fire = 80, held_item = ""

**Expected plan:**
1. Navigate to potato → Dig up potato
2. Navigate to campfire → Cook potato
3. Eat cooked potato

Agent completes full resource transformation chain.

---

### Scenario 2: Critical Fire vs Critical Hunger
**State:** Hunger = 95, Fire fuel = 15, held_item = ""

**Expected plan:**
1. Navigate to potato → Dig up potato
2. Drop potato if needed
3. Navigate to wood pile → Pick up wood
4. Navigate to campfire → Add fuel (fire now at 45)
5. Navigate back to potato (or dig new one)
6. Navigate to campfire → Cook potato
7. Eat cooked potato

OR if fire has just enough fuel:
1. Navigate to potato → Dig up potato
2. Navigate to campfire → Cook potato (uses remaining fire)
3. Eat cooked potato
4. Then handle fire maintenance

Agent must balance immediate hunger with fire dependency.

---

### Scenario 3: Fire Too Low to Cook
**State:** Hunger = 60, Fire fuel = 10, held_item = "potato"

**Expected plan:**
1. Drop potato
2. Navigate to wood pile → Pick up wood
3. Navigate to campfire → Add fuel
4. Navigate back to potato → Pick up potato
5. Navigate to campfire → Cook potato
6. Eat cooked potato

Agent recognizes cooking is blocked and must refuel first.

---

### Scenario 4: Preemptive Fire Maintenance
**State:** Hunger = 20, Fire fuel = 35, held_item = ""

**Expected plan:**
1. Navigate to wood pile → Pick up wood
2. Navigate to campfire → Add fuel (fire now at 65)
3. Navigate to potato → Dig up potato
4. Navigate to campfire → Cook potato
5. Eat cooked potato

Agent maintains fire proactively before it becomes critical, enabling smooth cooking workflow.

---

## File Locations

**New demo scenes:**
- `examples/campfire_2d.tscn`

**New shared gameplay code for the 2D-first implementation:**
- `examples/behaviors/campfire/`
  - `hunger_goal.gd`
  - `fire_maintenance_goal.gd`
  - `hunger_updater.gd`
  - `campfire_behavior_config.gd`
  - `eat_held_food_action.gd`
  - `drop_item_action.gd`
- `examples/objects/wood_pile/`
  - `wood_pile_object.gd`
  - `pick_up_wood_action.gd`
- `examples/objects/campfire/`
  - `campfire_object.gd`
  - `add_fuel_action.gd`
  - `cook_potato_action.gd`
- `examples/objects/potato/`
  - `potato_object.gd`
  - `dig_potato_action.gd`
- `examples/shared/`
  - `potato_spawner.gd`

**New 2D prefabs:**
- `examples/source_2d/prefabs/wood_pile_2d.tscn`
- `examples/source_2d/prefabs/campfire_2d.tscn`
- `examples/source_2d/prefabs/potato_2d.tscn`
- `examples/source_2d/prefabs/agent_2d.tscn`

**Deferred to a later pass:**
- `examples/campfire_3d.tscn`
- `examples/source_3d/prefabs/wood_pile_3d.tscn`
- `examples/source_3d/prefabs/campfire_3d.tscn`
- `examples/source_3d/prefabs/potato_3d.tscn`
- `examples/source_3d/prefabs/agent_3d.tscn`

---

## Testing Checklist

**Fire Mechanics:**
- [ ] Fire fuel decays at expected rate (3/sec)
- [ ] Agent holding wood can add fuel to fire
- [ ] Agent not holding wood navigates to pile first
- [ ] Can't add fuel if fire is already full (100)
- [ ] Visual: label shows correct fuel percentage

**Cooking System:**
- [ ] Agent can dig up potato (sets held_item = "potato")
- [ ] Agent holding raw potato can cook it at campfire (if fuel >= 20)
- [ ] Cooking transforms held_item from "potato" to "cooked_potato"
- [ ] Can't cook if fire fuel < min_fuel_to_cook threshold
- [ ] Agent can use `EatHeldFoodAction` on cooked potato (reduces hunger by 50)
- [ ] Can't eat if not holding cooked potato

**Inventory Management:**
- [ ] Can't pick up wood while already holding something
- [ ] Can't pick up potato while already holding something
- [ ] Agent can drop held items when replanning requires free hands
- [ ] held_item correctly shows: "", "wood", "potato", or "cooked_potato"
- [ ] Agent labels display current held_item

**Potato Spawning:**
- [ ] PotatoSpawner spawns 5 initial potatoes at random locations
- [ ] Potatoes despawn when picked up
- [ ] Potatoes respawn at new random locations after delay
- [ ] Never more than max_potatoes active at once

**AI Planning:**
- [ ] Hungry agent prioritizes getting food when hunger > 70
- [ ] Agent completes full chain: dig potato → cook → eat
- [ ] Agent recognizes need to refuel fire before cooking
- [ ] Multiple agents can share fire maintenance duty
- [ ] Agent with low hunger + low fire balances both needs

---

## Future Enhancements (Out of Scope for v1)

- **Wood pile cooldown** — piles need time to "regrow" after pickup
- **Fire goes out visual** — if fuel reaches 0, campfire visuals turn off/dim significantly
- **Relight action** — requires holding wood + using tinderbox/flint item to restart dead fire
- **Multiple fires** — agents choose which fire to maintain based on proximity
- **Other cookable foods** — fish, meat, etc. with different cook times
- **Partial cooking** — food burns if left too long, or becomes partially cooked
- **Fuel consumption during cooking** — cooking uses some fire fuel, not just blocking threshold

---

## Estimated Implementation Time

**Shared Code (2D-first):**
- **Goals (Hunger + Fire Maintenance) + Updater + BehaviorConfig:** 1.5 hours
- **WoodPileObject + PickUpWoodAction:** 1 hour  
- **CampfireObject + AddFuelAction + CookPotatoAction:** 2 hours
- **PotatoObject + DigPotatoAction + EatHeldFoodAction + DropItemAction:** 2 hours
- **PotatoSpawner system:** 1 hour

**2D Demo:**
- **Prefabs (campfire, wood, potato, agent) with labels:** 1.5 hours
- **Demo scene assembly + PotatoSpawner setup:** 1 hour
- **Navigation setup:** 0.5 hours

**Testing + bug fixes:** 1.5-2 hours (2D first pass)

**Total:** ~9-10 hours for a solid 2D-first implementation

---

## Documentation Updates Needed

Update `examples/README.md` to add:

```markdown
## Campfire Cooking Demo

**Scenes:** 
- 2D: `examples/campfire_2d.tscn`

Agents must maintain a shared campfire while gathering and cooking food. Demonstrates resource transformation (raw potato → cooked potato) and multi-step planning chains.

**Gameplay loop:**
1. Gather wood from static piles to maintain fire fuel
2. Dig up raw potatoes from randomized spawn locations
3. Cook potatoes at campfire (requires minimum fire fuel)
4. Eat cooked potatoes to reduce hunger

**Concepts demonstrated:**
- **Resource transformation** — cooking converts raw food to edible food
- **Object-based properties** — fire tracks its own fuel, agents query it directly
- **Multi-step preparation chains** — dig → cook → eat requires planning ahead
- **Competing priorities** — balance personal hunger vs group fire maintenance
- **Dynamic spawning** — potatoes respawn at randomized locations
- **Inventory management** — single held_item slot for wood/potato/cooked_potato
- **Task switching** — drop held items when priorities change
- **Threshold-based actions** — cooking requires fire fuel >= 20
- **2D-first implementation** — shared logic designed for later 3D extension
```
