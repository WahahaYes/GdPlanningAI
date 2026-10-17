# GdPlanningAI Examples

The `examples/` folder ships seven runnable demo scenes. This page is the map: how to run them, the order to read them in, and the ideas they share. Each script's class doc says what it is for.

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

## Reading Order

Each scene adds one idea to the previous one.

1. **hunger_basic_2d.tscn**: one agent, one need. A behavior config bundles a goal, actions and a property updater; hunger slowly outranks wandering; a plan is Go To plus an interaction. Open `behaviors/hunger/`, `behaviors/wander/`, `objects/fruit_tree/`, `objects/holdable/`.
1. **hunger_multi_agent_2d.tscn**, **hunger_stress_test_2d.tscn**: more agents on the same prefab. Agents plan independently and contention is accepted (see below). The stress test's spawner sets `agent_count = 5`; raise it to load the planner. Open `shared/many_agents_spawner.gd`.
1. **campfire_2d.tscn**: two goals and a held item. Requirements and provisions chained through a single held-item slot, objects that offer their own actions, and an infinite cost used as a gate. Open `behaviors/campfire/`, `objects/campfire/`, `objects/wood_pile/`, `objects/potato/`.
1. **campfire_3d.tscn**: the same logic in 3D. Only the body, navigation avoidance and navmesh setup change. Open `source_3d/assets/agent/nav_controller_3d.gd`.
1. **campfire_large_3d.tscn**: six agents, three fires. The spawner's `agents_per_campfire = 2` gives each pair a home fire by pointing their `MaintainFireGoal` at a per-fire group. Tab / Shift+Tab switch the followed agent. Open `shared/many_agents_spawner_3d.gd`, `behaviors/campfire/maintain_fire_goal.gd`.
1. **agent_showcase_3d.tscn**: presentation only. Planning is off; agents show skins, animation and carried props. Open `source_3d/assets/agent/`.

## Key Ideas

**Behavior configs.** An agent config (`examples/configs/*.tres`) lists behavior configs; the agent merges their goals, self actions and property updaters. `hunger_wander_agent_config.tres` combines Hunger and Wander, `campfire_agent_config.tres` Hunger and Campfire. Hunger registers the Go To; Wander and Campfire rely on it. Configs are merged without deduplication, so a second Go To would double navigation branching.

**Objects offer actions.** Anything that can be done at a place is offered by the object there (`get_provided_actions`), so adding a campfire or a potato to a scene needs no agent change.

**Three kinds of need.**

- *Validity checks* decide whether an action is offered at all, and may read live scene state (is the potato still there, is the tree on cooldown).
- *Preconditions* are checked against the simulated state at that point in the plan (`held_item == ""` before picking up).
- *Requirements and provisions* are the links the planner searches backward through. Add Fuel requires a `held_item == "wood"` binding; Pick Up Wood provides it.

An action often states one need both ways: the requirement drives discovery, the precondition guards correctness.

**Go To and `at_target`.** Every interaction requires `fact("at_target", [location])`. Only `GoToAction` provides it, as a wildcard bound to the requested location, so navigation is planned in automatically and one plan can contain several Go To steps. `WanderAction` extends Go To but provides no `at_target`.

**Food.** Pick-up provides `is_food` only for objects in the `Food` group, and Eat Held Food requires it, so wood can never be eaten. Eat Held Food also only accepts items in its restore table, which lists `cooked_potato` but not a raw `potato`.

**Infinite cost as a gate.** Cook Potato costs `INF` below the fire's cook threshold, and Add Fuel costs `INF` when the fire is full. This is where the hunger and fire goals meet.

**Contention.** There are no reservations. Two agents may plan for the same item; the second to arrive fails and replans. A game that needs fairness would add a reservation that the validity check consults.

## How Plans Form

Plans form backward from the goal through requirements and provisions.

```
Banana:        GoTo → Pick Up Item → Eat Held Food
Tree shake:    GoTo → Shake Tree                    (cost 10, so real food wins)
Fire:          GoTo → Pick Up Wood → GoTo → Add Fuel
Cook and eat:  GoTo → Dig Potato → GoTo → Cook Potato → Eat Held Food
```

Shake Tree's simulated effect lowers hunger directly instead of modelling the fruit that falls; the next plan picks the fruit up. The fire goal asks for a small rise per plan rather than the full threshold, so a nearly dead fire is always one delivery from progress. The campfire config uses `max_recursion = 6` (hunger config: 4) because its chains are longer.

## 3D Navigation

- Everything that shapes the navmesh sits under `NavigationRegion3D`. Campfires and wood piles carry a `StaticBody3D` and a `NavigationObstacle3D` with `affect_navigation_mesh` on, so they carve the baked mesh and agents path to its edge. Scenery bodies are carved by their colliders.
- After moving a prop, re-bake the region; the mesh is stored in the scene.
- Navigation avoidance keeps agents apart: the nav controllers send the wanted velocity to the server and apply the safe velocity it returns. Each agent's avoidance radius is its collision shape plus about 10%.
- The 2D bodies have collisions off and rely on avoidance alone.
