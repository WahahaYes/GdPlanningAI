# GdPlanningAI Examples

The `examples/` folder ships five runnable demo scenes that exercise the planning framework end to end. This document covers how to run them, what each scene demonstrates, and how the behavior modules, object types, and config resources behind them wire together.

## Running the Examples

The project has no main scene set, so demos are launched individually with Play Scene.

1. Open the repository root as a project in Godot 4.x.
1. Enable the addon: Project → Project Settings → Plugins, then toggle "GdPlanningAI" on.
1. In the FileSystem dock, open `examples/`. The five demo scenes sit at the top level.
1. Double-click a scene and press **Play Scene** (F6). The main Play button (F5) won't start anything, since no main scene is configured.

The folder splits dimension-agnostic code from dimension-specific presentation:

```
examples/
├── behaviors/    Goals, actions, property updaters, behavior configs (no 2D/3D code)
├── objects/      GdPAIObjectData subclasses and their interaction actions
├── shared/       Spawners and UI helpers
├── configs/      Agent config resources
├── source_2d/    2D prefabs, nav controllers, animator
├── source_3d/    3D prefabs and nav controller
└── *.tscn        The five runnable demo scenes
```

The `behaviors/`, `objects/`, `shared/`, and `configs/` folders are intentionally self-contained. You can copy `behaviors/hunger/` into your own project, rename the classes, and re-register the config to reuse the same logic.

## Demo Scenes

### hunger_basic_2d.tscn

The simplest demo, and the best place to start. A single blue monk agent lives in a tiled world and manages one blackboard property: hunger.

Scene tree, roughly:

```
SingleAgentDemo (root)
├── GdPAIWorldNode
├── Scenery            tiles, decorations, nav region
│   └── 5 × banana_tree.tscn
├── Agent              agent.tscn, hunger_wander_agent_config.tres
└── FPS display
```

What it demonstrates:

- A single agent planning and executing on its own.
- Hunger accumulating over time until the HungerGoal outranks the WanderGoal.
- Foraging for bananas and shaking fruit trees to get food.

### hunger_multi_agent_2d.tscn

The same tiled world, with two agents. The second agent reuses the same prefab with a red monk sprite override.

What it demonstrates:

- Two independent planners running side by side.
- Resource contention: both agents can chase the same banana tree or fruit pile, and the first to arrive wins.
- Goal prioritization drifting as each agent's hunger rises and falls independently.

### hunger_stress_test_2d.tscn

A performance test. `HugeScenery` packs 76+ banana trees into the world, and an AgentSpawner scatters agents across it.

One thing to note: the scene's AgentSpawner sets `agent_count = 5`, overriding the spawner's default of 20. Bump the value in the inspector to stress the planning engine harder. The FPS counter shows the per-frame cost as the count rises.

What it demonstrates:

- Planning engine performance under load.
- The spawner as a way to scatter many agents without hand-placing them.

### campfire_2d.tscn

The flagship demo. A single agent must balance two goals at once: keep the campfire burning and keep itself fed. This is where Requirements/Provisions, GoToAction chaining, and object-provided actions come into play.

Scene tree, roughly:

```
Campfire2D (root)
├── GdPAIWorldNode
├── NavigationRegion2D
├── Ground
├── Campfire              campfire_2d.tscn (CampfireRuntime + CampfireObject)
├── 4 × WoodPile          wood_pile_2d.tscn
├── PotatoSpawner         Rect2 area, respawns potatoes
├── Agent1                agent_2d.tscn, campfire_agent_config.tres
└── Camera2D
```

Food is sparser here than in the hunger scenes, and preparation is multi-step: dig a potato, cook it at the campfire, eat it. Meanwhile the fire decays every frame and needs wood. The planner reconciles these goals at runtime.

What it demonstrates:

- Two goals (maintain fire, reduce hunger) competing for the same agent.
- Requirements and Provisions: how the planner chains GoTo and interaction actions through the single held-item slot.
- The INF-cost gating that ties cooking to fire level: the planner won't plan to cook unless it also plans to keep the fire above the cook threshold.
- Object-provided actions: the campfire, wood pile, and potato each broadcast the actions the agent can use on them.

### campfire_3d.tscn

The same multi-goal scenario in 3D. The logic is identical; only the presentation differs: NavigationRegion3D, CSG meshes, and 3D prefabs (`campfire_3d.tscn`, `wood_pile_3d.tscn`, `potato_3d.tscn`, `agent_3d.tscn`). The agent is a CharacterBody3D driven by `nav_controller_3d.gd` instead of a RigidBody2D.

What it demonstrates:

- That the dimension-agnostic behavior and object code runs unchanged in 3D.
- The same two-goal planning, with a body that steers in 3D space.

## Behavior Modules

Behavior modules group a config, goals, actions, and property updaters into one reusable package. Each `GdPAIBehaviorConfig` subclass declares its own exported parameters and registers its pieces in `_populate`.

### Hunger (behaviors/hunger/)

The core survival loop. Hunger is a single blackboard property, `hunger`, on a 0 to 100 scale where 0 means full and 100 means starving.

**HungerBehaviorConfig** exports:

| Parameter | Default | Meaning | |-----------|---------|---------| | `hunger_decay` | 1.25 | Points of hunger added per second | | `initial_hunger` | 0.0 | Hunger at startup | | `hunger_restored_by_item` | {banana: 20.0, cooked_potato: 50.0} | Hunger restored per item | | `eat_duration` | 1.5 | Seconds the EatHeldFoodAction takes | | `optimistic_unbound_restore` | 20.0 | Restore assumed when planning for an unbound held item |

`_populate` registers:

- **HungerGoal**: reward is `max(0, hunger)`, so it climbs as the agent starves. Desired state asks for `hunger` below `max(0, current - 15)`. Note that reward is the current hunger value itself, 0 when full and 100 when starving.
- **GoToAction**: the agent's own navigation action.
- **EatHeldFoodAction**: defined in `behaviors/campfire/` but registered by the hunger config. Valid only when the agent has a `hunger` property. Requires `binding_exists("held_item")` and `fact("is_food", [])`, so it only eats things the world marks as food.
- **HungerPropertyUpdater**: `initialize` seeds the starting hunger; `update_properties` accumulates `min(100, hunger + decay * delta)` every frame. Hunger rises over time, it does not decay. The config's `hunger_decay` of 1.25 overrides the updater class default of 5.0.

### Wander (behaviors/wander/)

Gives a full agent something to do when no goal is urgent.

**WanderBehaviorConfig** exports `wander_distance = 256.0` and registers:

- **WanderGoal**: fixed reward of 5.0 whenever the agent's simulated position has moved far enough from the wander origin. (The docstring and the examples README claim 10; the code returns 5.0.)
- **WanderAction**: extends GoToAction, so no separate navigation action is needed. It simulates a random step of `wander_distance` in a random direction and records `wander_origin`; at execution it spawns a temporary Node2D/Node3D target, wraps it in a `GdPAILocationData`, navigates to it, then frees the temp node.

Wander deliberately provides no `at_target` fact. Only a real GoToAction satisfies `at_target`, which keeps wander from satisfying interaction action requirements during planning.

### Campfire (behaviors/campfire/)

The multi-goal module, designed to sit alongside HungerBehaviorConfig.

**CampfireBehaviorConfig** exports:

| Parameter | Default | Meaning | |-----------|---------|---------| | `fire_goal_reward` | 40.0 | Reward for the maintain-fire goal | | `desired_fuel_level` | 60.0 | Fuel threshold the goal targets | | `drop_duration` | 0.2 | Seconds the DropItemAction takes |

`_populate` registers:

- **MaintainFireGoal**: reward of `fire_goal_reward` if any CampfireObject proxy has `current_fuel < desired_fuel_level` (or if no campfire is found), otherwise 0. Desired state is `world_object_property_geq_than("CampfireObject", "current_fuel", 60)`.
- **GoToAction**: navigation.
- **DropItemAction**: precondition `held_item != ""`; clears the held-item binding. With a single held-item slot, dropping frees the slot so the planner can re-plan around it.

The `fire_goal_reward` of 40 is what lets the fire goal beat the hunger goal while hunger is still low: hunger reward only reaches 40 once hunger is above 40.

## Object Types

Objects are `GdPAIObjectData` subclasses. Each broadcasts a set of group labels and the actions it provides. The planner discovers these at plan time, so a scene can add objects without touching agent code.

| Object | Groups | Provides | Key sim properties | |--------|--------|----------|--------------------| | FoodObject (extends HoldableObject) | FoodObject, Food, HoldableObject, GdPAIObjectData | PickupAction (inherited) | hunger_value = 20 | | PotatoObject | PotatoObject, Food, GdPAIObjectData | DigPotatoAction | none | | HoldableObject | HoldableObject, GdPAIObjectData | PickupAction | item_id | | WoodPileObject | WoodPileObject, GdPAIObjectData | PickUpWoodAction | none | | FruitTreeObject | FruitTreeObject, GdPAIObjectData | ShakeTreeAction | none | | CampfireObject | CampfireObject, GdPAIObjectData | AddFuelAction, CookPotatoAction | fuel_per_wood = 30, current_fuel = 100, min_fuel_to_cook = 20 |

The "Food" group label is the load-bearing one. PickupAction provisions `fact("is_food", [])` only when the object is in the Food group, so bananas count as edible and wood does not. EatHeldFoodAction then requires that `is_food` fact, which is how the agent ends up able to eat only what the world explicitly marks as food.

The actions, briefly:

- **PickupAction**: requires `held_item == ""` and `fact("at_target", [object_location])`. Provisions `binding("held_item", item_id)` plus `is_food` if the object is in the Food group. Cost 1.0. Consumes the item on perform.
- **PickUpWoodAction**: like pickup, but provisions `binding("held_item", "wood")` with no `is_food`. Cost 0.5. The wood pile is not freed, so it works as an infinite source.
- **ShakeTreeAction**: cost 10.0, so direct food is preferred whenever it exists. Valid only when hunger is above 0 and the tree isn't on cooldown. Simulates a hunger reduction; on perform it spawns 1 to 3 fruit prefabs after a 0.5 second shake.
- **AddFuelAction**: requires `binding_equals("held_item", "wood")` and `at_target`. Raises fuel by `fuel_per_wood` (capped at 100) and clears the held item. Its cost becomes INF when the proxy is invalid or fuel is already at 100, which stops the planner from over-stocking the fire.
- **CookPotatoAction**: requires `held_item == "potato"` and `at_target`. Provisions `binding("held_item", "cooked_potato")` plus `is_food`, so a cooked potato feeds the EatHeldFoodAction chain. Cost 2.0, but INF when fuel is below `min_fuel_to_cook`; the real cook also fails at runtime if the fire has since gone low.
- **CampfireRuntime**: not an action, but the prefab root script. Decays fuel each frame (`max(0, current_fuel - fuel_decay_rate * delta)`), updates a "Campfire (N%)" label, and scales the fire sprite by fuel ratio.

## Config Resources

The two agent configs live in `examples/configs/`. Both use interval-based planning.

**hunger_wander_agent_config.tres** (used by `agent.tscn`):

- `planning_strategy = 1` (ON_INTERVAL), `planning_interval = 1.0`
- `max_recursion = 4`
- behaviors: HungerBehaviorConfig, WanderBehaviorConfig

**campfire_agent_config.tres** (used by `agent_2d.tscn` and `agent_3d.tscn`):

- `planning_strategy = 1` (ON_INTERVAL), `planning_interval = 1.0`
- `max_recursion = 6`
- behaviors: HungerBehaviorConfig, CampfireBehaviorConfig

The campfire config needs the deeper recursion because its plans are longer (GoTo → Dig → GoTo → Cook → Eat) and the planner must look further ahead to chain through the held-item slot.

`planning_strategy = 1` maps to ON_INTERVAL in the addon's `PlanningStrategy` enum, meaning the agent replans every second while it isn't already executing a plan. Neither config carries a blackboard plan; the empty `GdPAIBlackboardPlan` sub-resource lives on each scene's GdPAIWorldNode instead of in the configs.

## Shared Infrastructure

Utility scripts in `examples/shared/` keep the demos self-contained:

- **many_agents_spawner.gd**: scatters a configurable `agent_scene` across a `spawn_rect`. Exports `agent_count` (default 20) and `hide_debug_labels`. Locks agent rotation and hides the debug label per agent. The stress test overrides `agent_count` to 5.
- **agent_debug_label.gd / agent_debug_label_3d.gd**: per-frame on-screen text in the form "Goal: X / Action: Y / prop: val". A `debug_properties` array selects which blackboard properties to show: `["hunger"]` in the hunger scenes, `["hunger", "held_item"]` in the campfire scenes.
- **fps_counter.gd**: draws "FPS: n / Avg: n.n", useful for judging the stress test.
- **potato_spawner_2d.gd / potato_spawner_3d.gd**: keeps a renewable potato supply inside a spawn area. Exports `potato_scene`, `max_potatoes` (default 5), and `respawn_time` (default 10).
- **nav_controller.gd** (2D): steers the RigidBody2D along the NavigationAgent2D path at speed 128 with an arrival threshold of 8.
- **nav_controller_3d.gd**: moves a CharacterBody3D at speed 3.0 via `move_and_slide`.
- **agent_animator.gd**: picks Run/Idle from velocity and flips the sprite.

## How the Plans Are Formed

Plans form backwards. The agent picks a goal, then walks its desired state backwards through action provisions and requirements until the earliest action has no unmet needs. Every interaction action requires an `at_target` fact for its location, and only GoToAction provides it, which is how navigation slots into the front of every chain.

The `held_item` binding and the `is_food` fact are what the planner threads between actions. A binding satisfies `binding_exists` and `binding_equals` requirements; the `is_food` fact is provided only by actions and objects that produce food.

### Banana chain (hunger_basic_2d)

```
HungerGoal
  ← EatHeldFoodAction   requires binding_exists("held_item") + fact("is_food", [])
    ← PickupAction      provisions binding("held_item", "banana") + is_food
                        requires held_item == "" + fact("at_target", [banana])
      ← GoToAction      provides at_target
```

Result: GoTo → Pick Up Item → Eat Held Food.

### Tree-shake chain (hunger_basic_2d)

```
HungerGoal
  ← ShakeTreeAction     requires fact("at_target", [tree])
                        validity: hunger > 0, tree valid, tree not on cooldown
      ← GoToAction      provides at_target
```

Result: GoTo → Shake Tree. ShakeTreeAction's cost of 10 versus PickupAction's 1 is what tips the planner toward real food when both are reachable.

### Maintain-fire chain (campfire_2d)

```
MaintainFireGoal        wants current_fuel >= 60
  ← AddFuelAction       requires binding_equals("held_item", "wood") + at_target
                        cost INF when fuel >= 100
    ← PickUpWoodAction  provisions binding("held_item", "wood")  (no is_food)
                        requires held_item == "" + at_target
      ← GoToAction      provides at_target
```

Result: GoTo → Pick Up Wood → Add Fuel. The INF cost when the fire is already full is what keeps the agent from hauling wood to a stocked fire.

### Cook-and-eat chain (campfire_2d)

```
HungerGoal
  ← EatHeldFoodAction   cooked_potato restores 50
    ← CookPotatoAction  provisions binding("held_item", "cooked_potato") + is_food
                        requires binding_equals("held_item", "potato") + at_target
                        cost INF when fuel < 20
      ← DigPotatoAction provisions binding("held_item", "potato") + is_food
                        requires at_target
        ← GoToAction    provides at_target
```

Result: GoTo → Dig Potato → GoTo → Cook Potato → Eat Held Food. The INF cost on CookPotatoAction when fuel is low is the hinge that ties the two goals together: the planner cannot produce a cook-and-eat plan unless it also has a way to keep the fire hot enough, so hunger and fire maintenance get reconciled in a single plan.

## Known Doc Drift

A few comments and README claims don't match the code:

- WanderGoal's reward is a fixed 5.0. Its docstring and the examples README both say 10.
- Hunger accumulates over time. The README describes the hunger value as decaying, which is the inverse of what HungerPropertyUpdater does. Reward equals the current hunger value: 0 means full, 100 means starving.
- The stress test scene sets `agent_count = 5` on its spawner; the spawner default is 20.
