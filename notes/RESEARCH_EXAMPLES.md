# Research: Examples / Demo Setup

**Date**: 2026-08-01 **Source**: `examples/` — audited via explore agent (bg_ecaa1f8b) **Status**: Research dump — material for docs/EXAMPLES.md

______________________________________________________________________

## Directory layout

```
examples/
├── README.md
├── hunger_basic_2d.tscn          # single agent, simplest
├── hunger_multi_agent_2d.tscn    # two independent planners
├── hunger_stress_test_2d.tscn    # spawner perf test
├── campfire_2d.tscn              # 2D multi-goal campfire
├── campfire_3d.tscn              # 3D multi-goal campfire
├── behaviors/                    # dimension-agnostic
│   ├── hunger/    (behavior_config, goal, property_updater)
│   ├── wander/    (behavior_config, goal, action)
│   └── campfire/  (behavior_config, maintain_fire_goal, eat_held_food_action)
├── objects/                      # dimension-agnostic
│   ├── food/ food_object.gd
│   ├── potato/ potato_object.gd, dig_potato_action.gd
│   ├── fruit_tree/ fruit_tree_object.gd, shake_tree_action.gd
│   ├── holdable/ holdable_object.gd, pickup_action.gd, drop_item_action.gd
│   ├── wood_pile/ wood_pile_object.gd, pick_up_wood_action.gd
│   └── campfire/ campfire_object.gd, campfire_runtime.gd,
│                 add_fuel_action.gd, cook_potato_action.gd
├── shared/
│   ├── many_agents_spawner.gd
│   ├── ui/ agent_debug_label.gd, agent_debug_label_3d.gd, fps_counter.gd
│   └── systems/potato_spawner/ potato_spawner_2d.gd, potato_spawner_3d.gd
├── configs/
│   ├── hunger_wander_agent_config.tres
│   └── campfire_agent_config.tres
├── source_2d/                    # prefabs + movement scripts
│   ├── assets/agent/ nav_controller.gd, agent_animator.gd
│   └── prefabs/ agent.tscn, agent_2d.tscn, scenery.tscn, huge_scenery.tscn,
│                banana_tree.tscn, banana.tscn, campfire_2d.tscn,
│                wood_pile_2d.tscn, potato_2d.tscn
└── source_3d/
    ├── assets/agent/ nav_controller_3d.gd
    └── prefabs/ agent_3d.tscn, campfire_3d.tscn, wood_pile_3d.tscn, potato_3d.tscn
```

## Demo scenes (all runnable with Play Scene / F6)

1. **hunger_basic_2d.tscn** (root SingleAgentDemo): GdPAIWorldNode + Scenery (tiles, decorations, 5 banana trees, nav region) + one blue-monk Agent (agent.tscn, hunger_wander config) + FPS display.
1. **hunger_multi_agent_2d.tscn** (MultiAgentDemo): same world, two agents (blue + red monk sprite override) — resource contention + independent planning.
1. **hunger_stress_test_2d.tscn** (ManyAgentsDemo): HugeScenery (76+ banana trees) + AgentSpawner (agent_count=5 in scene; spawner default 20) + FPS HUD.
1. **campfire_2d.tscn** (Campfire2D): GdPAIWorldNode + NavigationRegion2D + Ground + 1 Campfire + 4 WoodPiles + PotatoSpawner (Rect2 area) + Agent1 (agent_2d.tscn, campfire config) + Camera2D. Demonstrates Requirements/ Provisions, GoTo chaining, object actions.
1. **campfire_3d.tscn** (Campfire3D): same logic, 3D — NavigationRegion3D, CSG visuals, campfire_3d.tscn, wood_pile_3d, potato_3d, agent_3d.tscn (CharacterBody3D).

## Behavior modules

### Hunger (behaviors/hunger/)

- **HungerBehaviorConfig**: exports `hunger_decay=1.25`, `initial_hunger=0.0`, `hunger_restored_by_item={"banana":20.0,"cooked_potato":50.0}`, `eat_duration=1.5`, `optimistic_unbound_restore=20.0`. `_populate` adds HungerGoal, GoToAction, EatHeldFoodAction, HungerPropertyUpdater.
- **HungerGoal**: `reward = max(0, hunger)` (0 full → 100 starving); desired state `agent_property_less_than("hunger", max(0, current-15))`.
- **HungerPropertyUpdater**: `initialize` seeds hunger; `update_properties` **accumulates** hunger (`min(100, hunger + decay*delta)`) — hunger rises over time (class default decay 5.0; config's 1.25 wins).

### Wander (behaviors/wander/)

- **WanderBehaviorConfig**: `wander_distance=256.0`; adds WanderGoal + WanderAction. No GoToAction (WanderAction IS a GoToAction).
- **WanderGoal**: reward `5.0` fixed (⚠️ docstring + README claim 10 — code returns 5.0). Desired state: custom precondition — simulated location moved
  > req_distance (16.0 Vector2 / 1.0 Vector3) from wander_origin.
- **WanderAction extends GoToAction**: validity `entity` + `GdPAILocationData`; cost 1.0; **provisions `[]`** (comment: only the agent's GoToAction provides at_target); simulate moves sim position by wander_distance in random dir + records wander_origin; pre_perform creates a temp Node2D/3D target + wraps in GdPAILocationData, then super() navigation; post frees temp node.

### Campfire (behaviors/campfire/)

- **CampfireBehaviorConfig**: exports `fire_goal_reward=40.0`, `desired_fuel_level=60.0`, `drop_duration=0.2`; adds MaintainFireGoal, GoToAction, DropItemAction. Used alongside HungerBehaviorConfig.
- **MaintainFireGoal**: reward = 40 if any CampfireObject proxy has `current_fuel < desired_fuel_level` (or none found), else 0. Desired state: `world_object_property_geq_than("CampfireObject", "current_fuel", 60)`.
- **EatHeldFoodAction** (in campfire folder, registered by HungerConfig): validity `agent_has_property("hunger")`; preconditions `hunger > 0` + custom `_is_holding_food` (held_item in map); requirements `binding_exists("held_item")` + `fact("is_food", [])`; cost eat_duration; simulate reduces hunger by restore (or optimistic_unbound_restore when unbound) + clears held_item.

## Object types (each broadcasts get_provided_actions)

| Object | Groups | Provides | Key sim props | |--------|--------|----------|---------------| | FoodObject (extends HoldableObject) | FoodObject, Food, HoldableObject, GdPAIObjectData | (inherits PickupAction) | hunger_value=20 | | PotatoObject | PotatoObject, Food, GdPAIObjectData | DigPotatoAction | {} | | HoldableObject | HoldableObject, GdPAIObjectData | PickupAction | item_id | | WoodPileObject | WoodPileObject, GdPAIObjectData | PickUpWoodAction | — | | FruitTreeObject | FruitTreeObject, GdPAIObjectData | ShakeTreeAction | — | | CampfireObject | CampfireObject, GdPAIObjectData | AddFuelAction + CookPotatoAction | fuel_per_wood=30, current_fuel=100, min_fuel_to_cook=20 |

- **PickupAction**: validity object-valid; precondition `held_item==""`; requirement `fact("at_target", [object_location])`; provisions `binding("held_item", item_id)` + `fact("is_food", [])` **only if object in "Food" group** (bananas edible, wood not); cost 1.0; perform frees item.
- **DropItemAction** (campfire config): precondition `held_item != ""`; clears held_item — unblocks replanning with the single held-item slot.
- **PickUpWoodAction**: precondition `held_item==""`; requirement at_target; provision `binding("held_item","wood")` (no is_food); cost 0.5; pile NOT freed (infinite source).
- **ShakeTreeAction** (SHAKE_DURATION 0.5, cost **10.0** — prefers real food): validity agent hunger > 0 + tree valid + custom tree-not-on-cooldown check; requirement at_target; simulate reduces hunger by `hunger_value * drop_min_amount` (optimistic); perform spawns 1-3 fruit prefabs (drop_fruit) after 0.5s.
- **AddFuelAction** (1.0s): precondition `held_item=="wood"`; requirements `binding_equals("held_item","wood")` + at_target; cost 1.0 (INF if proxy invalid or fuel >= 100); simulate raises fuel by fuel_per_wood (cap 100) + clears held_item — "Action-Led Hypothetical Progress": reports effect even when wood binding unsatisfied during discovery.
- **CookPotatoAction** (2.0s): validity object-valid; precondition `held_item=="potato"`; requirements `binding_equals("held_item","potato")` + at_target; provisions `binding("held_item","cooked_potato")` + `fact("is_food",[])` (restores 50); cost 2.0 (INF if fuel < min_fuel_to_cook); perform FAILURE if real fuel too low at runtime.
- **CampfireRuntime** (prefab root): per-frame fuel decay (`max(0, current_fuel - fuel_decay_rate*delta)`), label "Campfire (N%)", fire scale by fuel_ratio.

## Config resources (examples/configs/)

- **hunger_wander_agent_config.tres**: `planning_strategy=1`, `planning_interval=1.0`, `max_recursion=4`; behaviors [HungerBehaviorConfig, WanderBehaviorConfig]. Used by agent.tscn.
- **campfire_agent_config.tres**: `planning_strategy=1`, `planning_interval=1.0`, `max_recursion=6`; behaviors [HungerBehaviorConfig, CampfireBehaviorConfig]. Used by agent_2d.tscn + agent_3d.tscn.
- ⚠️ Neither sets a blackboard plan — the scene's GdPAIWorldNode carries an (empty) GdPAIBlackboardPlan sub-resource. `planning_strategy=1` = ON_INTERVAL (interval field present; ordinal 1 per addon enum).

## Shared infrastructure

- **many_agents_spawner.gd**: `agent_scene`, `agent_count=20`, `spawn_rect`, `hide_debug_labels=true`; instantiates + scatters agents, locks rotation, hides DebugLabel.
- **agent_debug_label.gd / \_3d**: per-frame "Goal: X\\nAction: Y\\n<prop>: val"; `debug_properties` array (agent.tscn: ["hunger"]; agent_2d/3d: ["hunger","held_item"]).
- **fps_counter.gd**: "FPS: %s\\nAvg: %.1f".
- **potato_spawner_2d/3d.gd**: `potato_scene`, spawn area, `max_potatoes=5`, `respawn_time=10`; keeps renewable potato supply.
- **nav_controller.gd** (2D): steers RigidBody2D toward NavigationAgent2D path, speed 128, arrival_threshold 8.
- **agent_animator.gd**: Run/Idle by velocity, flips sprite.
- **nav_controller_3d.gd**: CharacterBody3D, speed 3.0, move_and_slide.

## End-to-end wiring traces (for EXAMPLES.md)

- **Trace A — banana**: HungerGoal ← EatHeldFoodAction (binding_exists held_item + fact is_food) ← PickupAction (binding held_item="banana" + is_food, requires held_item=="" + at_target) ← GoToAction. Plan: GoTo → Pick Up Item → Eat Held Food.
- **Trace B — tree shake**: HungerGoal ← ShakeTreeAction (validity hunger/tree/ cooldown; requires at_target; simulates hunger reduction) ← GoToAction. Cost 10 vs 1 makes direct food preferred when available.
- **Trace C — maintain fire**: MaintainFireGoal (fuel ≥ 60) ← AddFuelAction (binding_equals held_item="wood" + at_target) ← PickUpWoodAction (binding held_item="wood") ← GoToAction. Plan: GoTo → Pick Up Wood → Add Fuel.
- **Trace D — cook & eat**: HungerGoal ← EatHeldFoodAction (cooked_potato 50) ← CookPotatoAction (provides held_item="cooked_potato" + is_food; requires binding_equals held_item="potato" + at_target; INF cost if fuel < 20) ← DigPotatoAction (provides held_item="potato" + is_food) ← GoToAction. Plan: GoTo → Dig Potato → GoTo → Cook Potato → Eat Held Food. The INF-cost gating links hunger and fire maintenance.

## Key mechanics

- `fact("at_target", [location_data])` on every interaction action; satisfied only by GoToAction (WanderAction explicitly does not provide it).
- `fact("is_food", [])` marks edibles; wood deliberately omits it.
- `binding("held_item", value)` / binding_exists / binding_equals drive the single held-item slot.
- No `FactWildcard` symbol exists in examples — the wildcard-style `at_target` fact + `is_food` fact pattern is used (the go-to provision class used by GoToAction is `ProvisionSpec.fact_wildcard` in the addon itself).

## Discrepancies to flag in docs

1. WanderGoal.compute_reward() returns **5.0**, docstring + README claim 10.
1. HungerPropertyUpdater **increases** hunger over time; README/docstring say "decays". README also words hunger reward as scaling "0→100 as hunger drops"; code rewards hunger directly (0=full, 100=starving).
1. hunger_behavior_config default hunger_decay 1.25 vs HungerPropertyUpdater class default 5.0 (config wins).
1. hunger_stress_test scene sets agent_count=5, not spawner default 20.
