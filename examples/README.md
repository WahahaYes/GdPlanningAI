# GdPlanningAI Examples

______________________________________________________________________

## Setup

1. Open this project in Godot 4.x
1. Enable the GdPlanningAI addon if not already enabled:
   - Project → Project Settings → Plugins
   - Enable "GdPlanningAI"
1. Navigate to `examples/` in the FileSystem dock - **demo scenes are at the top level!**

______________________________________________________________________

## Running Examples

**To run:** Open any `.tscn` file at the `examples/` root and press **Play Scene** (F6). The project has no main scene, so F5 does nothing.

New here? Read in this order; `docs/EXAMPLES.md` explains what each step teaches and which scripts to open:

1. `hunger_basic_2d.tscn`
1. `hunger_multi_agent_2d.tscn` and `hunger_stress_test_2d.tscn`
1. `campfire_2d.tscn`
1. `campfire_3d.tscn`
1. `campfire_large_3d.tscn`
1. `agent_showcase_3d.tscn` (presentation only)

### 2D Demos

- **`hunger_basic_2d.tscn`** — Basic single-agent hunger management

  - One agent foraging for food and shaking fruit trees
  - Good starting point to understand core concepts

- **`hunger_multi_agent_2d.tscn`** — Multiple agents competing for resources

  - Two agents with independent planning
  - Demonstrates resource contention and goal prioritization

- **`hunger_stress_test_2d.tscn`** — Performance stress test

  - Many agents (configurable spawner; the scene sets 5, the spawner default is 20)
  - Tests planning engine performance under load

- **`campfire_2d.tscn`** — Multi-goal planning with object interactions

  - Agent maintains campfire fuel while managing hunger
  - Demonstrates requirements and provisions, GoToAction chaining, and object-provided actions
  - Uses `CampfireBehaviorConfig` + `HungerBehaviorConfig` combined

______________________________________________________________________

### 3D Demos

- **`campfire_3d.tscn`** — 3D version of the campfire demo

  - Same multi-goal logic in a 3D scene
  - Campfires and wood piles carve the baked navigation mesh so agents path around them
  - Agents steer with navigation avoidance

- **`campfire_large_3d.tscn`** — Many agents, several fires

  - Six agents with different skins, three campfires, four wood piles
  - Each pair of agents tends its own home campfire
  - Camera follows one agent; Tab and Shift+Tab switch

- **`agent_showcase_3d.tscn`** — Presentation showcase

  - Skins, walking and idle animation, and carried props
  - Planning is switched off; not an example of planning logic

______________________________________________________________________

## Folder Organization

**Demo scenes** are at the root for easy access (`hunger_basic_2d.tscn`, etc.)

**Dimension-agnostic code:**

- `behaviors/` - Goals, PropertyUpdaters, BehaviorConfigs
- `objects/` - ObjectData + Actions
- `shared/` - Utility scripts (spawners, debug labels, FPS counter)
- `configs/` - Agent configuration resources

**Dimension-specific source files:**

- `source_2d/` - 2D prefabs, nav controller, animator
- `source_3d/` - 3D prefabs, nav controller, agent skin, carry visual, camera rig, showcase driver

______________________________________________________________________

## Concept Map

- `behaviors/hunger/` — `Goal`, `PropertyUpdater`, `GdPAIBehaviorConfig`
- `behaviors/wander/` — `Goal`, `Action`, `GdPAIBehaviorConfig`
- `behaviors/campfire/` — `Goal`, `Action`, `GdPAIBehaviorConfig` (multi-goal with Hunger)
- `objects/food/` — `GdPAIObjectData`, interaction actions
- `objects/fruit_tree/` — `GdPAIObjectData`, interaction actions, validity checks with external state
- `objects/wood_pile/` — `GdPAIObjectData`, `PickUpWoodAction`, requirements and provisions
- `objects/holdable/` — `GdPAIObjectData`, `PickupAction`, `DropItemAction`
- `objects/campfire/` — `GdPAIObjectData`, `AddFuelAction`, `CookPotatoAction`, `CampfireRuntime`
- `objects/potato/` — `GdPAIObjectData`, `DigPotatoAction`

______________________________________________________________________

## The Demo Scenario

The hunger demos place agents in a tiled world containing **fruit trees** and fruit. Each agent has a hunger value that rises over time (0 is full, 100 is starving). Its goals and priorities work as follows:

- **Hunger goal** — reward equals the current hunger value, so it grows from 0 to 100 as the agent starves. The more hungry the agent, the more it prefers to seek food or shake a tree.
- **Wander goal** — fixed reward of 5. Wins while hunger is below 5, or when no food-related actions are reachable.

The planner chains actions together: e.g. navigate to tree → shake tree → navigate to fallen fruit → pick it up → eat it. Interaction cost is a small constant and navigation cost grows with distance, so agents prefer the closest option.

The campfire demos add a second goal, **Maintain Fire** (fixed reward of 40), and a longer food chain: dig a potato, cook it at the fire, eat it.

Agents do not claim or reserve targets. Two agents can race for the same item and the loser replans; the examples accept that to stay simple.

______________________________________________________________________

## Using Behaviors and Objects in Your Own Project

The files in `behaviors/` and `objects/` are dimension-agnostic and intentionally self-contained. To adapt them:

1. Copy the relevant folder (e.g., `behaviors/hunger/`) into your project.
1. Rename the class (e.g., `HungerGoal` → `EnergyGoal`) and adjust the blackboard property name and reward formula for your domain.
1. Register your `BehaviorConfig` subclass in a `GdPAIAgentConfig` resource.

The `source_2d/` and `source_3d/` folders contain dimension-specific prefabs and assets. The `shared/` folder contains dimension-agnostic utility scripts (UI helpers, spawners). The runnable demos at the root reference these source files.
