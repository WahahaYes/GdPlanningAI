# GdPlanningAI Examples

The `examples/` folder ships seven runnable demo scenes that exercise the planning framework end to end. This document covers how to run them, a suggested reading order, what each scene demonstrates, and how the behavior modules, object types and config resources behind them fit together. The example scripts carry their own class docs explaining why each piece is built the way it is; this page is the map.

## Running the Examples

The project has no main scene set, so demos are launched individually with Play Scene.

1. Open the repository root as a project in Godot 4.x.
1. Enable the addon: Project → Project Settings → Plugins, then toggle "GdPlanningAI" on.
1. In the FileSystem dock, open `examples/`. The demo scenes sit at the top level.
1. Double-click a scene and press **Play Scene** (F6). The main Play button (F5) won't start anything, since no main scene is configured.

The folder splits dimension-agnostic code from dimension-specific presentation:

```
examples/
├── behaviors/    Goals, actions, property updaters, behavior configs (no 2D/3D code)
├── objects/      GdPAIObjectData subclasses and their interaction actions
├── shared/       Spawners and UI helpers
├── configs/      Agent config resources
├── source_2d/    2D prefabs, nav controller, animator
├── source_3d/    3D prefabs, nav controller, agent skin, carry visual, camera rig, showcase driver
└── *.tscn        The runnable demo scenes
```

The `behaviors/`, `objects/`, `shared/`, and `configs/` folders are intentionally self-contained. You can copy `behaviors/hunger/` into your own project, rename the classes, and re-register the config to reuse the same logic.

## Suggested Reading Order

Read the scenes in this order. Each step adds one idea to the previous one, and each names the scripts worth opening while it runs.

1. **hunger_basic_2d.tscn: the smallest complete agent.** One agent, one need. Teaches how a behavior config bundles a goal, actions and a property updater, how a goal's reward competes with another goal, and how a plan is a chain of Go To and an interaction. Open `behaviors/hunger/`, `behaviors/wander/`, then `objects/fruit_tree/` and `objects/holdable/`.
1. **hunger_multi_agent_2d.tscn and hunger_stress_test_2d.tscn: more agents.** The same prefab several times. Teaches that agents plan independently, that contention is accepted rather than managed (see Contention below), and what planning costs at scale. Open `shared/many_agents_spawner.gd`.
1. **campfire_2d.tscn: two goals and a held item.** Teaches multi-goal agents, requirements and provisions chained through the single held-item slot, objects that provide their own actions, and an action cost of infinity used as a gate. Open `behaviors/campfire/`, `objects/campfire/`, `objects/wood_pile/`, `objects/potato/`.
1. **campfire_3d.tscn: the same logic in 3D.** Teaches that the planning code does not change; what changes is the body (a `CharacterBody3D`), navigation avoidance, and how static objects are made navigable around. Open `source_3d/assets/agent/nav_controller_3d.gd` and the campfire and wood pile prefabs.
1. **campfire_large_3d.tscn: many agents, several fires.** Teaches how a spawner can wire agents to scene objects at runtime (home fires), and why the fire goal asks for one delivery at a time. Open `shared/many_agents_spawner_3d.gd` and `behaviors/campfire/maintain_fire_goal.gd`.
1. **agent_showcase_3d.tscn: presentation only.** Planning is switched off; three agents show skins, walking and idle animation, and carried props. Teaches nothing about planning, but shows where art ends and logic begins. Open `source_3d/assets/agent/` and `source_3d/assets/debug/showcase_driver_3d.gd`.

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

The first multi-goal demo. A single agent must balance two goals at once: keep the campfire burning and keep itself fed. This is where requirements and provisions, GoToAction chaining, and object-provided actions come into play.

Scene tree, roughly:

```
Campfire2D (root)
├── GdPAIWorldNode
├── Terrain
│   ├── Ground
│   ├── NavigationRegion2D
│   ├── Camera2D
│   ├── Campfire              campfire_2d.tscn (CampfireRuntime + CampfireObject)
│   ├── 4 × WoodPile          wood_pile_2d.tscn
│   └── PotatoSpawner         Rect2 area, respawns potatoes
└── Agent1                    agent_2d.tscn, campfire_agent_config.tres
```

Food is sparser here than in the hunger scenes, and preparation is multi-step: dig a potato, cook it at the campfire, eat it. Meanwhile the fire burns down every frame and needs wood. The planner reconciles these goals at runtime.

What it demonstrates:

- Two goals (maintain fire, reduce hunger) competing for the same agent.
- Requirements and provisions: how the planner chains GoTo and interaction actions through the single held-item slot.
- The INF-cost gating that ties cooking to fire level: the planner won't plan to cook unless the fire is above the cook threshold.
- Object-provided actions: the campfire, wood pile, and potato each broadcast the actions the agent can use on them.

### campfire_3d.tscn

The same multi-goal scenario in 3D, on a single-agent 20 by 20 meter ground with one campfire at the center, four wood piles, scenery, and a potato spawner. The planning logic is identical; only the presentation differs. The agent is a `CharacterBody3D` driven by `nav_controller_3d.gd` instead of a `RigidBody2D`.

What it demonstrates:

- That the dimension-agnostic behavior and object code runs unchanged in 3D.
- How static objects interact with navigation (see "3D scene structure" below).
- Avoidance-based steering, presentation helpers (carry visual, skin, camera rig) and what they do not do.

### campfire_large_3d.tscn

A 40 by 40 meter version with three campfires, four wood piles, scenery on both sides and eight potatoes. An AgentSpawner places six agents with different character skins. With `agents_per_campfire = 2` it gives each pair its own home fire (see "Home campfires" below). The camera follows one agent; Tab and Shift+Tab switch to another.

What it demonstrates:

- Many agents sharing resources with only independent planning.
- Per-agent configuration applied at runtime by a spawner.

### agent_showcase_3d.tscn

A small scene with three agents and scenery, used to inspect skins, animation and carried props. A `showcase_driver_3d.gd` on the root switches each agent to on-demand planning (so nothing is planned), sets a held item (potato, wood, empty), and walks them to random points. It is not an example of planning.

## Behavior Modules

Behavior modules group a config, goals, actions, and property updaters into one reusable package. Each `GdPAIBehaviorConfig` subclass declares its own exported parameters and registers its pieces in `_populate`.

### How a config bundles and merges behaviors

A `GdPAIAgentConfig` (the `.tres` files in `examples/configs/`) holds a list of behavior configs. When the agent starts it calls `apply_to_agent` on each one in order. Each config's `_populate` fills three lists: goals, self actions (actions that belong to the agent, like Go To or Eat Held Food), and property updaters. The agent appends all three to its own lists. Updaters also get to initialize their blackboard property at this point, so the property exists before the first plan.

The result is that adding a concern to an agent means adding a behavior config to its agent config, not editing any agent script. The two configs shipped here are:

- `hunger_wander_agent_config.tres`: Hunger plus Wander. Goals: Hunger and Wander. Self actions: Go To, Eat Held Food, Wander.
- `campfire_agent_config.tres`: Hunger plus Campfire. Goals: Hunger and Maintain Fire. Self actions: Go To, Eat Held Food, Go To, Drop Item.

Go To appears twice in the second list because each behavior config registers its own, so a config also works when used alone.

Actions the agent does not own come from the world: every object in the scene with a `GdPAIObjectData` node tells the planner which actions it offers (see Object Types). Goals decide what the agent wants, self actions say what it can do anywhere, and objects say what can be done at specific places.

### Preconditions, requirements and provisions

Three mechanisms describe what an action needs, and the examples use all three on purpose.

- **Validity checks** (`get_validity_checks`) decide whether the action is offered at all. They run when the planner collects actions and may inspect live scene state: is the potato still there, is the tree on cooldown, does the agent even have a hunger property.
- **Preconditions** (`get_preconditions`) are tested against the simulated state at that point in the plan. `held_item == ""` for picking something up is a precondition: it checks the simulated agent, nothing else.
- **Requirements and provisions** (`get_requirements`, `get_provisions`) are the planner-readable links between actions. A requirement says what an earlier action must have provided; a provision says what this action provides to a later one. They are what let the planner search backwards: to use Add Fuel it needs a `held_item == "wood"` binding and an `at_target` fact, and it looks for actions that provide them.

An action often states the same need as both a precondition and a requirement. Add Fuel's precondition `held_item == "wood"` checks the simulated state; its requirement `binding_equals("held_item", "wood")` tells the planner which earlier action to schedule. The first guards correctness, the second drives discovery.

### Go To and the wildcard at_target

Every interaction action (pick up, dig, add fuel, cook, shake a tree) requires `fact("at_target", [location])` for the object it acts on. Nobody provides that fact except `GoToAction`, which provides a wildcard `at_target`: a fact that satisfies any `at_target` requirement regardless of which location it names. The planner binds the concrete location when it satisfies a requirement and injects it into a fresh copy of the Go To action.

That is why navigation shows up automatically at the front of every chain, and why the same Go To can appear several times in one plan with different targets:

```
Go To (wood pile) → Pick Up Wood → Go To (campfire) → Add Fuel
```

The first Go To satisfies Pick Up Wood's `at_target`; the second satisfies Add Fuel's. `WanderAction` extends Go To but deliberately provides no `at_target`, so wandering can never stand in for walking to an object.

### Hunger (behaviors/hunger/)

The core survival loop. Hunger is a single blackboard property, `hunger`, on a 0 to 100 scale where 0 means full and 100 means starving. It rises over time and only eating lowers it.

**HungerBehaviorConfig** exports:

- `hunger_decay` (1.25): points of hunger added per second.
- `initial_hunger` (0.0): hunger at startup.
- `hunger_restored_by_item` (banana 20, cooked_potato 50): hunger restored per item.
- `eat_duration` (1.5): seconds the EatHeldFoodAction takes.
- `optimistic_unbound_restore` (20.0): restore assumed when planning for a held item the planner has not yet bound.

`_populate` registers:

- **HungerGoal**: reward is `max(0, hunger)`, so it climbs as the agent starves. The desired state asks for hunger below `max(15, current - 15)`: one meal's worth of improvement, never below the sated level. Asking for one meal at a time keeps the goal reachable in a single plan.
- **GoToAction**: the agent's own navigation action.
- **EatHeldFoodAction**: defined in `behaviors/campfire/` but registered by the hunger config. It is a self action because it acts on the held item, not a world object. Valid only when the agent has a `hunger` property. It requires `binding_exists("held_item")` and `fact("is_food", [])`, so the planner must find an earlier action that provides food.
- **HungerPropertyUpdater**: `initialize` seeds the starting hunger; `update_properties` accumulates `min(100, hunger + decay * delta)` every frame. The config's `hunger_decay` of 1.25 overrides the updater class default of 5.0.

### Wander (behaviors/wander/)

Gives a full agent something to do when no goal is urgent.

**WanderBehaviorConfig** exports `wander_distance = 256.0` and registers:

- **WanderGoal**: fixed reward of 5.0 whenever the agent's simulated position has moved far enough from the wander origin (16 in 2D, 1 in 3D).
- **WanderAction**: extends GoToAction, so no separate navigation action is needed. It simulates a random step of `wander_distance` in a random direction and records `wander_origin`; at execution it spawns a temporary Node2D/Node3D target, wraps it in a `GdPAILocationData`, navigates to it, then frees the temp node.

### Campfire (behaviors/campfire/)

The multi-goal module, designed to sit alongside HungerBehaviorConfig.

**CampfireBehaviorConfig** exports:

- `fire_goal_reward` (40.0): reward for the maintain-fire goal.
- `desired_fuel_level` (60.0): fuel threshold the goal targets.
- `campfire_group` (empty): group of the campfire(s) this agent tends. Empty tends any campfire.
- `drop_duration` (0.2): seconds the DropItemAction takes.

`_populate` registers:

- **MaintainFireGoal**: reward of `fire_goal_reward` if any tended campfire has `current_fuel < desired_fuel_level` (or if no campfire is found), otherwise 0. The desired state is not the full threshold; it asks only for the fire to rise a little above its current best level, capped at the threshold. One wood delivery always satisfies that, so a nearly dead fire stays recoverable: asking for the full 60 from a low fire would need several delivery rounds in one plan, more than the planner searches.
- **GoToAction**: navigation.
- **DropItemAction**: precondition `held_item != ""`; clears the held-item binding. With a single held-item slot, dropping frees the slot so the planner can re-plan around it.

The `fire_goal_reward` of 40 is what lets the fire goal beat the hunger goal while hunger is still low: hunger reward only reaches 40 once hunger is above 40.

## Object Types

Objects are `GdPAIObjectData` subclasses. Each broadcasts a set of group labels and the actions it provides. The planner discovers these at plan time, so a scene can add objects without touching agent code.

- **FoodObject** (extends HoldableObject): groups FoodObject, Food, HoldableObject, GdPAIObjectData. Provides PickupAction (inherited). Sim property `hunger_value = 20`.
- **HoldableObject**: groups HoldableObject, GdPAIObjectData. Provides PickupAction. Sim property `item_id`.
- **PotatoObject**: groups PotatoObject, Food, GdPAIObjectData. Provides DigPotatoAction. No sim properties.
- **WoodPileObject**: groups WoodPileObject, GdPAIObjectData. Provides PickUpWoodAction. No sim properties.
- **FruitTreeObject**: groups FruitTreeObject, GdPAIObjectData. Provides ShakeTreeAction. No sim properties; the cooldown is live scene state read by a validity check.
- **CampfireObject**: groups CampfireObject, GdPAIObjectData. Provides AddFuelAction and CookPotatoAction. Sim properties `fuel_per_wood = 30`, `current_fuel = 50` at the start of both campfire scenes (the planner works from a copy), `min_fuel_to_cook = 20`.

The "Food" group label is the load-bearing one. PickupAction provisions `fact("is_food", [])` only when the object is in the Food group, so bananas count as edible and wood does not. EatHeldFoodAction then requires that `is_food` fact, which is how the agent ends up able to eat only what the world explicitly marks as food.

The actions, briefly:

- **PickupAction**: precondition `held_item == ""`; requirement `fact("at_target", [object_location])`. Provisions `binding("held_item", item_id)` plus `is_food` if the object is in the Food group. Cost 1.0. Frees the item on perform.
- **PickUpWoodAction**: like pickup, but provisions `binding("held_item", "wood")` with no `is_food`. Cost 0.5. The wood pile is not freed, so it works as an infinite source, and nothing about it needs tracking in the simulation.
- **DigPotatoAction**: like pickup for a potato. Provisions `held_item == "potato"`. Cost 0.7. Frees the potato.
- **ShakeTreeAction**: cost 10.0, so direct food is preferred whenever it exists. Valid only when hunger is above 0 and the tree isn't on cooldown. Simulates a hunger reduction in place of modelling the fruit that falls; on perform it spawns 1 to 3 fruit prefabs after a 0.5 second shake and the next plan picks them up.
- **AddFuelAction**: requires `binding_equals("held_item", "wood")` and `at_target`. Raises fuel by `fuel_per_wood` (capped at 100) and clears the held item. Its cost becomes INF when the proxy is invalid or fuel is already at 100, which stops the planner from over-stocking the fire.
- **CookPotatoAction**: requires `held_item == "potato"` and `at_target`. Provisions `binding("held_item", "cooked_potato")` plus `is_food`, so a cooked potato feeds the EatHeldFoodAction chain. Cost 2.0, but INF when fuel is below `min_fuel_to_cook`; the real cook also fails at runtime if the fire has since gone low.
- **CampfireRuntime**: not an action, but the prefab root script. Burns fuel down each frame (`max(0, current_fuel - fuel_decay_rate * delta)`), updates a "Campfire (N%)" label, and scales the fire visual by fuel ratio (hiding it when it is out). It is world behavior, not planning.

Why the campfire, wood pile and potato each provide actions instead of the agent knowing about them: the agent only knows its self actions. Anything that can be done at a place is offered by the thing at that place, so adding a second campfire or another potato needs no change to an agent or config.

## Contention the Examples Accept

There is no claim or reservation logic. If two agents want the same potato, banana or tree, both plan to walk to it. The first to arrive takes it; the other finds it gone (the action's validity check fails, or `perform_action` returns failure) and replans. The same holds for a fuel level that changes between planning and acting: the plan was right when made, so the action fails and the agent plans again. This keeps the examples small and shows planning that tolerates a changing world. A game that needs fairness or efficiency would add a reservation on the target and have the validity check consult it.

## 3D Scene Structure

The 3D scenes build a navigation mesh in the editor and bake it; a few structural choices make agents walk around objects correctly.

- **NavigationRegion3D holds everything that shapes the mesh.** The ground, walls, scenery and the interactable props are all children of the region, so the bake sees them. Objects outside it would not affect the mesh.
- **Campfires and wood piles carry a StaticBody3D collider and a NavigationObstacle3D.** The obstacle has `affect_navigation_mesh` on and avoidance off: it exists to carve a hole in the baked mesh so the path planner treats the prop as unwalkable, with a footprint a little larger than the collider. An agent sent to a fire's location therefore paths to the closest reachable point on the edge of the hole, where its navigation finishes, rather than trying to walk into the middle. (If the agent stalls a short way from its target, Go To treats it as arrived when it is within a couple of target-distances of the end of its path.) The collider is the backstop: it keeps the physical body out if steering or avoidance ever pushes it toward the prop, and it blocks other agents. Avoidance is switched off on the obstacle because it is not an avoiding agent; the mesh carve already handles it.
- **Scenery rocks, ruins and barrels are StaticBody3D under the region.** They have colliders, and the bake picks those up, so they are carved out of the mesh without needing an obstacle. They are decoration with physical presence: they exist so the large scene reads as a place and so paths bend around something.
- **Re-bake after moving anything.** The baked mesh is stored in the scene file. If you move a campfire, wood pile or rock, bake the region again or agents will path through the old position.
- **Ground and walls** are a CSG box with collision and four wall bodies that keep agents in the arena.

The 2D scenes do not need this: 2D agent bodies have physics collisions switched off and avoid each other only through navigation avoidance.

## Navigation and Avoidance

Both nav controllers (`source_2d/assets/agent/nav_controller.gd`, `source_3d/assets/agent/nav_controller_3d.gd`) turn the path from the `NavigationAgent` into motion. With avoidance enabled they do not move the body directly. They hand the wanted velocity to the navigation agent and apply the safe velocity it returns on `velocity_computed`. The server computes that safe velocity from the other agents' velocities, so agents heading for the same pile or fire slide past each other instead of clumping and pushing.

The avoidance radius on each agent scene is the body's collision shape plus roughly 10 percent: 0.55 for the 0.5 meter cylinder in 3D, 18 for the 16 pixel circle on the campfire 2D agent, and 36 for the 64 pixel square on the hunger 2D agent. A radius at or below the body lets agents overlap; much larger makes them keep unnecessary distance. The controller also sets `max_speed` on the nav agent from its own `speed`, so the server never asks for more than the body can do.

Avoidance separates agents from agents. Static props are handled by the navmesh carve described above.

## Home Campfires in the Large Scene

`campfire_large_3d.tscn` has three campfires and six agents. The spawner's `agents_per_campfire = 2` assigns spawned agents to campfires in order: agents one and two to the first fire, three and four to the second, five and six to the third. For each fire it adds the campfire to a group (`HomeCampfire1`, `HomeCampfire2`, ...) and points the paired agents' MaintainFireGoal at that group. Each pair therefore tends one fire and ignores the others, so wood hauling is spread over the scene and no single fire is crowded.

Because each plan asks for the fire to rise by one wood delivery (not for the full threshold), a pair can restore a nearly dead fire one trip at a time. The flat reward keeps both agents delivering until the fire reaches the threshold. Hunger still competes for each agent independently, so which agent hauls wood at any moment varies.

## Presentation Helpers

Several scripts exist only to make the demo readable. None of them affect planning:

- **Debug labels** (`agent_debug_label.gd`, `agent_debug_label_3d.gd`): show the current goal, plan step and chosen blackboard properties.
- **Carry visual** (`carry_visual_3d.gd`): mirrors the `held_item` property as a floating prop.
- **Agent skin and animator** (`agent_skin_3d.gd`, `agent_animator.gd`): play idle and walk clips from the body's velocity.
- **Camera rig** (`camera_follow_3d.gd`): follow, orbit, zoom and agent switching.
- **Showcase driver** (`showcase_driver_3d.gd`): drives the showcase scene without planning.
- **FPS counter** and the **spawners**: scene scaffolding.

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
- **many_agents_spawner_3d.gd**: the 3D version. Scatters agents in a box, cycles `skins`, hands the camera rig an agent to follow, and builds home campfires (see above).
- **agent_debug_label.gd / agent_debug_label_3d.gd**: per-frame on-screen text in the form "Goal: X / Action: Y / prop: val". A `debug_properties` array selects which blackboard properties to show: `["hunger"]` in the hunger scenes, `["hunger", "held_item"]` in the campfire scenes.
- **fps_counter.gd**: draws "FPS: n / Avg: n.n", useful for judging the stress test.
- **potato_spawner_2d.gd / potato_spawner_3d.gd**: keeps a renewable potato supply inside a spawn area. Exports `potato_scene`, `max_potatoes` (default 5) and `respawn_time` (default 10). The 3D version also keeps potatoes out of a radius around each campfire, since a potato in the carved area could not be reached.
- **nav_controller.gd** (2D): steers the RigidBody2D along the NavigationAgent2D path at speed 128 with an arrival threshold of 8.
- **nav_controller_3d.gd**: moves a CharacterBody3D at speed 3.0 via `move_and_slide`, with acceleration, turning and gravity.
- **agent_animator.gd**: picks Run/Idle from velocity and flips the sprite.

## How the Plans Are Formed

Plans form backwards. The agent picks a goal, then walks its desired state backwards through action provisions and requirements until the earliest action has no unmet needs. Every interaction action requires an `at_target` fact for its location, and only GoToAction provides it, which is how navigation slots into the front of every chain.

The `held_item` binding and the `is_food` fact are what the planner threads between actions. A binding satisfies `binding_exists` and `binding_equals` requirements; the `is_food` fact is provided only by actions that produce food.

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

### Maintain-fire chain (campfire scenes)

```
MaintainFireGoal        wants current_fuel to rise (up to 60)
  ← AddFuelAction       requires binding_equals("held_item", "wood") + at_target
                        cost INF when fuel >= 100
    ← PickUpWoodAction  provisions binding("held_item", "wood")  (no is_food)
                        requires held_item == "" + at_target
      ← GoToAction      provides at_target
```

Result: GoTo → Pick Up Wood → GoTo → Add Fuel. The INF cost when the fire is already full is what keeps the agent from hauling wood to a stocked fire.

### Cook-and-eat chain (campfire scenes)

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

Result: GoTo → Dig Potato → GoTo → Cook Potato → Eat Held Food. The INF cost on CookPotatoAction when fuel is low is the hinge that ties the two goals together: the planner cannot produce a cook-and-eat plan for a cold fire, so hunger and fire maintenance get reconciled by the fire goal keeping the fire hot. A raw potato cannot be eaten directly: Eat Held Food only accepts items in its restore table, and that lists the cooked potato, not the raw one.

## Known Doc Drift

A few comments and README claims did not match the code and have been corrected:

- WanderGoal's reward is a fixed 5.0, not 10.
- Hunger accumulates over time; it does not decay. Reward equals the current hunger value: 0 means full, 100 means starving.
- The stress test scene sets `agent_count = 5` on its spawner; the spawner default is 20.
