# GdPlanningAI — Examples Reorganization Plan

## Goals

1. **Teach by structure.** The directory layout itself should communicate GdPAI concepts
   (`Goal`, `Action`, `GdPAIObjectData`, `PropertyUpdater`, `BehaviorConfig`). A user who
   opens the `examples/` folder for the first time should immediately understand which file to
   read to learn which concept.

2. **Separate reusable building blocks from demo scenes.** The old structure mixed
   GDScript class definitions (hunger goal, wander action, etc.) with runnable scenes and
   visual assets in a flat, topic-named layout. The new structure separates what you
   *learn from* (`shared/`) from what you *run* (`demo_2d/`).

3. **Remove obsolete content.** The old examples contained multithreading demo scenes and
   a threaded agent config that no longer apply after the synchronous Rust migration.
   Integration test scenes (`simple_bridge_test`, `rust_bridge_test`) are not examples and
   should not live here.

4. **Clean up naming.** Classes have a `Sample` prefix (e.g. `SampleFoodAction`) which is
   redundant inside an `examples/` folder. Rename to the plain concept name
   (e.g. `FoodAction`). This also avoids polluting the user's project namespace with
   `Sample*` class names when they import the addon.

5. **Document the structure.** Add a `README.md` at the root of `examples/` that maps the
   directory layout to GdPAI concepts.

---

## Audit of `old_examples`

### What exists

| Path | Contents | Disposition |
|---|---|---|
| `hunger/` | `SampleHungerGoal`, `SampleFoodAction`, `SampleFoodObject`, `HungerPropertyUpdater`, `HungerBehaviorConfig` | Port to `shared/` |
| `wander/` | `SampleWanderGoal`, `SampleWanderAction`, `WanderBehaviorConfig` | Port to `shared/` |
| `fruit_tree/` | `SampleFruitTreeObject`, `SampleShakeTreeAction` | Port to `shared/` |
| `agent_configs/` | `base_agent_config.tres`, `threaded_agent_config.tres` | Port one; drop threaded |
| `2D/demo_scenes/` | `single_agent_demo.tscn`, `multi_agent_demo.tscn`, `multithread_single_agent.tscn`, `multithreading_stress_test.tscn`, `singlethreading_stress_test.tscn` | Port two; drop three |
| `2D/assets/` | Tileset, sprites, `sample_agent.tscn`, `scenery.tscn`, `banana.tscn`, `banana_tree.tscn`, nav/animator/ui scripts | Port to `demo_2d/` |
| `simple_bridge_test/` | Integration test scene | Drop |
| `rust_bridge_test/` | Integration test scene (already deleted) | Already gone |

### What to drop

- `threaded_agent_config.tres` — threading is no longer part of the architecture.
- `multithread_single_agent.tscn`, `multithreading_stress_test.tscn`,
  `singlethreading_stress_test.tscn` — same reason.
- `simple_bridge_test/` — not an example, was a regression test.

---

## Proposed New Structure

```
examples/
  README.md                          ← describes folder layout and how to run demos

  shared/                            ← reusable GdPAI building blocks
    behaviors/
      hunger/                        ← teaches: Goal, PropertyUpdater, BehaviorConfig
        hunger_goal.gd
        hunger_property_updater.gd
        hunger_behavior_config.gd
      wander/                        ← teaches: Goal, Action, BehaviorConfig
        wander_goal.gd
        wander_action.gd
        wander_behavior_config.gd
    objects/
      food/                          ← teaches: GdPAIObjectData + SpatialAction
        food_object.gd
        food_action.gd
      fruit_tree/                    ← teaches: GdPAIObjectData + SpatialAction + cooldown
        fruit_tree_object.gd
        shake_tree_action.gd

  demo_2d/                           ← runnable 2D demo scenes
    scenes/
      single_agent_demo.tscn
      multi_agent_demo.tscn
    assets/                          ← visual / scene prefabs
      agent/
        agent.tscn                   ← the reusable agent prefab
        nav_controller.gd            ← moves the RigidBody2D per NavigationAgent2D
        agent_animator.gd
      world/
        scenery.tscn
        huge_scenery.tscn
        tileset.tres
        banana.tscn
        banana_tree.tscn
        banana.png
        TinySwordsPack/
      ui/
        fps_counter.gd
        debug_label.gd
    configs/
      agent_config.tres              ← default GdPAIAgentConfig .tres
```

---

## Naming Changes

| Old class name | New class name | Rationale |
|---|---|---|
| `SampleHungerGoal` | `HungerGoal` | Drop redundant `Sample` prefix |
| `HungerPropertyUpdater` | `HungerPropertyUpdater` | Already clean |
| `HungerBehaviorConfig` | `HungerBehaviorConfig` | Already clean |
| `SampleFoodAction` | `FoodAction` | Drop prefix |
| `SampleFoodObject` | `FoodObject` | Drop prefix |
| `SampleWanderGoal` | `WanderGoal` | Drop prefix |
| `SampleWanderAction` | `WanderAction` | Drop prefix |
| `WanderBehaviorConfig` | `WanderBehaviorConfig` | Already clean |
| `SampleFruitTreeObject` | `FruitTreeObject` | Drop prefix |
| `SampleShakeTreeAction` | `ShakeTreeAction` | Drop prefix |
| `sample_2d_nav.gd` → `nav_controller.gd` | `NavController` | Descriptive rename |
| `sample_2d_agent_animator.gd` → `agent_animator.gd` | `AgentAnimator` | Descriptive rename |
| `sample_fps_counter.gd` → `fps_counter.gd` | `FPSCounter` | Descriptive rename |

---

## `README.md` Structure (for `examples/`)

The README should cover:
1. **How to run a demo** — open `demo_2d/scenes/single_agent_demo.tscn` and play.
2. **Folder map** — `shared/behaviors/` for agent-side logic, `shared/objects/` for world
   objects, `demo_2d/` for the playable scene.
3. **Concept map** — one-line description of which GdPAI class each subdirectory demonstrates.
4. **Note on reuse** — explain that `shared/` classes are intentionally self-contained and
   can be copied into a real project as starting points.

---

## Implementation Order

```
Step 1 — Create directory skeleton and README.md
Step 2 — Port and rename shared/behaviors/hunger/
Step 3 — Port and rename shared/behaviors/wander/
Step 4 — Port and rename shared/objects/food/
Step 5 — Port and rename shared/objects/fruit_tree/
Step 6 — Port demo_2d/assets/ (split into agent/, world/, ui/)
Step 7 — Port demo_2d/configs/ (drop threaded config)
Step 8 — Port and update demo_2d/scenes/ (drop threading scenes, fix all resource paths)
Step 9 — Delete old_examples/
```

Steps 2–5 can be done in any order. Step 8 depends on steps 2–7.

---

## Resolved Decisions

### `debug_label.gd` → `AgentDebugLabel`

`debug_label.gd` extends `Label` (a `Control` node — works in 2D and 3D scenes alike),
so the 2D qualifier is not warranted in the name. It becomes `AgentDebugLabel`.

The existing implementation also uses a stale API (`_current_plan.get_plan()` no longer
exists) and hardcodes `"Hunger:"` in its text format, making it demo-specific rather than
generally useful. The rewrite should:
- Use the current API: `agent._current_goal`, `agent._current_action_chain`,
  `agent._current_plan_step`
- Display only goal title and current action title (no domain-specific properties like
  hunger)
- Keep it as a plain `extends Label` node that any project can drop onto an agent

File location: `demo_2d/assets/ui/agent_debug_label.gd`

---

### `WanderAction` nav-agent cleanup → `NavigatingAction` base class

`WanderAction` duplicates nav-agent discovery logic that already lives on `SpatialAction`.
Rather than having `WanderAction` call `SpatialAction._find_nav_agent()` (odd cross-class
static reference) or duplicating the lookup again, introduce a thin
`NavigatingAction` base class in the framework (`scripts/refcounteds/navigating_action.gd`).

`NavigatingAction extends Action` and provides:
- `const ARRIVAL_THRESHOLD_2D: float = 8.0`
- `const ARRIVAL_THRESHOLD_3D: float = 0.1`
- `static func _find_nav_agent(entity: Node) -> Node`

`SpatialAction` then extends `NavigatingAction` (removing its own copy of the constants and
helper). `WanderAction` also extends `NavigatingAction` and calls `_find_nav_agent`
directly, removing the manual `GdPAIUTILS.get_child_of_type` branching in
`pre_perform_action`.

This is a framework-layer change that slightly expands the class hierarchy but eliminates
the duplication cleanly.

**Note:** The broader SpatialAction to GoToAction migration is documented in `GOTOACTION_PORT_TODO.md`. The `NavigatingAction` base class approach may be superseded by the GoToAction pattern.

---

### Assets sub-split — confirmed

The `demo_2d/assets/` folder splits into `agent/`, `world/`, and `ui/` as proposed.
