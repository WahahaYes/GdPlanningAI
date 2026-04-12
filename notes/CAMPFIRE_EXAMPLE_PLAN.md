# GdPlanningAI — Campfire Tending Example

## Prerequisites

**This example assumes examples have been moved to the top-level `examples/` folder.** See `EXAMPLES_REORGANIZATION.md` for migration steps if not yet completed.

---

## Goals

This example demonstrates **maintenance/proactive planning** — a planning pattern where agents must prevent resource depletion before it causes failure, rather than reactively responding to needs.

**Key Learning Outcomes:**
1. **Competing priorities** — balance personal needs (hunger) vs group needs (fire maintenance)
2. **Proactive planning** — gather fuel before fire dies, not after
3. **Carrying/inventory state** — simple item holding via blackboard boolean
4. **Multi-step preparation** — must acquire resources before using them
5. **Time-critical actions** — fire fuel decays, creating urgency
6. **Threshold-based goals** — goal reward scales as resource depletes

---

## Design Overview

### The Scenario

Agents gather around a campfire that provides warmth/cooking capability. The fire's fuel level decays over time. Agents must balance gathering wood from the forest and keeping the fire alive while also managing their own hunger.

**Fail state:** If fire fuel reaches 0, the fire goes out (blocks cooking actions, increases urgency dramatically).

**Success pattern:** Agents proactively maintain fuel levels by gathering wood and adding it to the fire before it becomes critical.

---

## Reused Infrastructure

This example leverages existing systems with minimal new code:

| Existing System | How We Use It |
|---|---|
| `PropertyUpdater` | `FireFuelUpdater` decays fuel over time (like `HungerPropertyUpdater`) |
| `SpatialAction` | Navigate to wood piles, navigate to campfire |
| `GdPAIObjectData` | `WoodPileObject`, `CampfireObject` provide actions |
| Property decay pattern | Fire fuel behaves like hunger (inverse: low = bad) |
| Timed interactions | `AddFuelAction` has brief duration like eating |
| Dynamic validity | Can't add fuel if not holding wood |
| Multi-step chains | Gather wood → navigate to fire → add fuel |

**Net new systems:** ~0. Just new goal/action/object implementations using existing patterns.

---

## New Components

### Behaviors

#### `shared/behaviors/fire_maintenance/`

**`fire_maintenance_goal.gd`** (`MaintainFireGoal extends Goal`)
```gdscript
# Reward scales from 0 (fire full) to 80 (fire nearly out)
# Competes with HungerGoal (0-100 range)
func compute_reward(agent: GdPAIAgent) -> float:
    var fuel: float = agent.blackboard.get_property("campfire_fuel")
    if fuel == null:
        return 0.0
    # Reward increases as fuel drops
    return max(0.0, 80.0 - fuel)

func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
    var current_fuel: float = agent.blackboard.get_property("campfire_fuel")
    # Want fire fuel to increase
    return [Precondition.agent_property_greater_than("campfire_fuel", current_fuel)]
```

**`fire_fuel_updater.gd`** (`FireFuelUpdater extends PropertyUpdater`)
```gdscript
var fuel_decay_rate: float = 3.0  # Fuel per second
var initial_fuel: float = 100.0

func initialize(agent: GdPAIAgent) -> void:
    agent.blackboard.set_property("campfire_fuel", initial_fuel)
    agent.blackboard.set_property("holding_wood", false)

func update_properties(agent: GdPAIAgent, delta: float) -> void:
    var fuel: float = agent.blackboard.get_property("campfire_fuel")
    agent.blackboard.set_property("campfire_fuel", max(0.0, fuel - fuel_decay_rate * delta))
```

**`fire_maintenance_behavior_config.gd`** (`FireMaintenanceBehaviorConfig extends GdPAIBehaviorConfig`)
```gdscript
func _populate(
    goals: Array[Goal],
    actions: Array[Action],
    updaters: Array[PropertyUpdater]
) -> void:
    goals.append(MaintainFireGoal.new())
    updaters.append(FireFuelUpdater.new())
```

---

### Objects

#### `shared/objects/wood_pile/`

**`wood_pile_object.gd`** (`WoodPileObject extends GdPAIObjectData`)
```gdscript
@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData

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
    checks.append(Precondition.agent_has_property("holding_wood"))
    # Can't pick up if already holding
    checks.append(Precondition.agent_property_equal_to("holding_wood", false))
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
    agent_blackboard.set_property("holding_wood", true)

func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
    var parent_status: Action.Status = super(agent, delta)
    if parent_status == Action.Status.FAILURE:
        return Action.Status.FAILURE
    
    if not get_state(agent, "target_reached"):
        return Action.Status.RUNNING
    
    # Instant pickup once at location
    agent.blackboard.set_property("holding_wood", true)
    return Action.Status.SUCCESS
```

---

#### `shared/objects/campfire/`

**`campfire_object.gd`** (`CampfireObject extends GdPAIObjectData`)
```gdscript
@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData
@export var fuel_per_wood: float = 30.0  # How much fuel one wood restores

func get_group_labels() -> Array[String]:
    return ["CampfireObject", "GdPAIObjectData"]

func get_provided_actions() -> Array[Action]:
    return [AddFuelAction.new(location_data, interactable_attribs, fuel_per_wood)]

func get_sim_properties() -> Dictionary:
    return {"fuel_per_wood": fuel_per_wood}
```

**`add_fuel_action.gd`** (`AddFuelAction extends SpatialAction`)
```gdscript
const ADD_FUEL_DURATION: float = 1.0
var fuel_per_wood: float

func _init(
    p_object_location: GdPAILocationData,
    p_interactable_attribs: GdPAIInteractable,
    p_fuel_per_wood: float
) -> void:
    super(p_object_location, p_interactable_attribs)
    fuel_per_wood = p_fuel_per_wood

func get_validity_checks() -> Array[Precondition]:
    var checks: Array[Precondition] = super()
    checks.append(Precondition.agent_has_property("holding_wood"))
    checks.append(Precondition.agent_has_property("campfire_fuel"))
    # Must be holding wood to add it
    checks.append(Precondition.agent_property_equal_to("holding_wood", true))
    # No point if fire is already full
    checks.append(Precondition.agent_property_less_than("campfire_fuel", 100.0))
    return checks

func get_action_cost(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> float:
    var cost: float = super(agent_blackboard, world_state)
    if cost == INF:
        return INF
    return cost + ADD_FUEL_DURATION

func simulate_effect(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard
) -> void:
    super(agent_blackboard, world_state)
    var fuel: float = agent_blackboard.get_property("campfire_fuel")
    agent_blackboard.set_property("campfire_fuel", min(100.0, fuel + fuel_per_wood))
    agent_blackboard.set_property("holding_wood", false)

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
        var fuel: float = agent.blackboard.get_property("campfire_fuel")
        agent.blackboard.set_property("campfire_fuel", min(100.0, fuel + fuel_per_wood))
        agent.blackboard.set_property("holding_wood", false)
        return Action.Status.SUCCESS
    
    return Action.Status.RUNNING

func post_perform_action(agent: GdPAIAgent) -> Action.Status:
    super(agent)
    erase_state(agent, "add_fuel_elapsed")
    return Action.Status.SUCCESS
```

---

## Demo Scene Setup

### 2D Version: `demo_2d/scenes/campfire_demo.tscn`

**World Layout:**
- Central campfire (visual: AnimatedSprite2D flame, shrinks as fuel drops)
- 3-4 wood pile locations scattered around map
- 2-3 fruit tree locations (existing food system)
- NavigationRegion2D covering playable area

**Agent Setup:**
- 2-4 agents (RigidBody2D + NavigationAgent2D)
- Both `HungerBehaviorConfig` and `FireMaintenanceBehaviorConfig`
- Each agent starts with `hunger = 100`, `campfire_fuel = 100`, `holding_wood = false`

### 3D Version: `demo_3d/scenes/campfire_demo.tscn`

**World Layout:**
- Central campfire (visual: GPUParticles3D + MeshInstance3D with glow material)
- 3-4 wood pile locations (simple cylinder meshes)
- 2-3 fruit tree locations (reuse 3D tree models if available, or simple primitives)
- NavigationRegion3D with baked NavMesh

**Agent Setup:**
- 2-4 agents (CharacterBody3D + NavigationAgent3D)
- Same behavior configs as 2D version
- Same starting properties

**Shared Blackboard:**
Agents share `campfire_fuel` property via world state (Solution B from Implementation Challenges)

---

## Implementation Challenges & Solutions

### Challenge 1: Shared Fire Fuel

**Problem:** Multiple agents track the same fire. How do they share fuel state?

**Solution A (Recommended):** Fire fuel is an **agent property** that references world state
- `FireFuelUpdater` reads from `world_state.get_property("global_fire_fuel")`
- Each agent mirrors it to local blackboard for planning
- Only one updater actually decrements (attached to campfire object)

**Solution B:** Fire fuel is purely world state
- Agents have `MaintainFireGoal` that checks world state
- Preconditions use `world_state_property_less_than("fire_fuel", 100)`
- Simpler but requires world state preconditions

**Decision:** Use **Solution B** — cleaner separation, teaches world state usage.

### Challenge 2: Wood Pile Respawn

**Problem:** Should wood piles deplete?

**Solution:** No depletion in v1. Piles are infinite. Keeps example simple and focuses on planning, not resource management. Can add cooldown in v2 if desired (reuse tree cooldown pattern).

### Challenge 3: Visual Feedback

**Problem:** How to show fire fuel level?

**Solution:** 
- Campfire has `Label` child showing fuel percentage
- Flame sprite `scale` tied to fuel (0.5 to 1.0 scale range)
- Particle system intensity/amount tied to fuel

---

## Planning Scenarios Demonstrated

### Scenario 1: Proactive Maintenance
**State:** Hunger = 50, Fire = 40, Holding wood = false

**Planner output:**
1. Navigate to wood pile → Pick up wood
2. Navigate to campfire → Add fuel
3. Navigate to fruit tree → Shake tree
4. Navigate to banana → Eat

Fire maintenance takes priority despite moderate hunger.

---

### Scenario 2: Critical Fire vs Critical Hunger
**State:** Hunger = 10, Fire = 15, Holding wood = false

**Planner output:**
1. Navigate to fruit/food → Eat (Hunger goal = 90 reward)
2. Navigate to wood pile → Pick up wood
3. Navigate to campfire → Add fuel

Survival trumps maintenance. Fire goal only = 85 reward.

---

### Scenario 3: Opportunistic Wood Gathering
**State:** Hunger = 100, Fire = 80, Holding wood = false, Wood pile nearby

**Planner output:**
1. Navigate to wood pile → Pick up wood (preemptive)
2. Wander

Even when fire isn't urgent, low-cost wood pickup might happen if agent is idle and nearby. Demonstrates cost-based opportunism.

---

### Scenario 4: Two-Step Planning
**State:** Hunger = 30, Fire = 20, Holding wood = false

**Expected plan:**
1. Navigate to wood pile → Pick up wood
2. Navigate to campfire → Add fuel
3. Navigate to food → Eat

Agents recognize they must get wood FIRST before they can add fuel. Can't directly satisfy fire goal without preparation step.

---

## File Structure

**Note:** Examples use flattened structure - demo scenes at root, dimension-agnostic code in `behaviors/`, `objects/`, `shared/`, and dimension-specific assets in `source_2d/` and `source_3d/`.

```
examples/
  # Runnable demos at root
  campfire_2d.tscn                  ← NEW: 2D campfire demo
  campfire_3d.tscn                  ← NEW: 3D campfire demo
  
  # Dimension-agnostic code
  behaviors/
    fire_maintenance/
      fire_maintenance_goal.gd
      fire_fuel_updater.gd
      fire_maintenance_behavior_config.gd
      fire_maintenance_behavior_config.tres
  
  objects/
    wood_pile/
      wood_pile_object.gd
      pick_up_wood_action.gd
    campfire/
      campfire_object.gd
      add_fuel_action.gd
  
  shared/                           ← dimension-agnostic utility scripts
    ui/
      fps_counter.gd
      agent_debug_label.gd
  
  configs/
    campfire_agent_config.tres      ← NEW: includes hunger + fire maintenance
  
  # 2D source files
  source_2d/
    prefabs/
      wood_pile.tscn                ← Node2D + WoodPileObject + Sprite2D
      campfire.tscn                 ← Node2D + CampfireObject + AnimatedSprite2D + Label
    assets/
      agent/                        ← 2D nav/animation controllers
      world/                        ← wood pile sprite, fire animations
  
  # 3D source files
  source_3d/
    prefabs/
      agent.tscn                    ← CharacterBody3D + NavigationAgent3D
      wood_pile.tscn                ← Node3D + WoodPileObject + MeshInstance3D
      campfire.tscn                 ← Node3D + CampfireObject + GPUParticles3D + Label3D
    assets/
      agent/                        ← 3D agent model
      world/                        ← wood pile model, fire particles
```

**All paths use:** `res://examples/behaviors/`, `res://examples/objects/`, `res://examples/shared/`, `res://examples/source_2d/`, etc.

**Note:** All GDScript files in `behaviors/` and `objects/` are dimension-agnostic and used by both 2D and 3D demos.

---

## Testing Checklist

- [ ] Single agent maintains fire above 50 fuel for 60 seconds
- [ ] Agent holding wood can add fuel to fire
- [ ] Agent not holding wood navigates to pile first
- [ ] Fire fuel decays at expected rate
- [ ] Multiple agents can share fire maintenance duty
- [ ] Hungry agent prioritizes food when hunger < 20
- [ ] Agent with full fire + full hunger wanders
- [ ] Wood pickup action sets `holding_wood = true`
- [ ] Add fuel action sets `holding_wood = false`
- [ ] Can't pick up wood while already holding wood
- [ ] Can't add fuel without holding wood
- [ ] Visual: fire shrinks as fuel depletes
- [ ] Visual: label shows correct fuel percentage

---

## Future Enhancements (Out of Scope for v1)

- **Wood pile cooldown** — piles need time to "regrow" after pickup
- **Cooking action** — requires fire fuel > threshold, produces cooked food
- **Fire goes out** — if fuel reaches 0, campfire becomes inactive until reignited
- **Relight action** — requires holding wood + tinderbox item
- **Multiple fires** — agents choose which fire to maintain based on proximity
- **Shared world state fire** — world-level fire that all agents reference

---

## Estimated Implementation Time

**Shared Code (dimension-agnostic):**
- **Goal + Updater + BehaviorConfig:** 1 hour
- **WoodPileObject + PickUpWoodAction:** 1 hour  
- **CampfireObject + AddFuelAction:** 1.5 hours

**2D Demo:**
- **Demo scene assembly:** 1 hour
- **Visual assets (sprites, particles, labels):** 1 hour

**3D Demo:**
- **Demo scene assembly:** 1.5 hours (navigation baking, lighting)
- **Visual assets (meshes, materials, particles):** 1.5 hours

**Testing + bug fixes:** 1.5 hours (both versions)

**Total:** ~9-10 hours for both 2D and 3D implementations

---

## Documentation Updates Needed

Update `examples/README.md` to add:

```markdown
## Campfire Tending Demo

**Scenes:** 
- 2D: `demo_2d/scenes/campfire_demo.tscn`
- 3D: `demo_3d/scenes/campfire_demo.tscn`

Agents must maintain a shared campfire by gathering wood and adding fuel before it burns out, while also managing their own hunger.

**Concepts demonstrated:**
- Maintenance goals with threshold-based rewards
- Proactive planning (gather resources before crisis)
- Simple inventory state (holding_wood boolean)
- Multi-step preparation chains (get wood → add fuel)
- Competing priorities (personal needs vs group needs)
- **Dimension-agnostic code** (same action/goal scripts work in both 2D and 3D)
```
