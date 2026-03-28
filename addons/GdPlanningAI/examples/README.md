# GdPlanningAI Examples

## How to Run

Open `demo_2d/scenes/single_agent_demo.tscn` or `demo_2d/scenes/multi_agent_demo.tscn`
in the Godot editor and press **Play Scene**.

---

## Folder Layout

```
examples/
  shared/                 ← reusable building blocks (read these to learn the API)
	behaviors/
	  hunger/             ← Goal, PropertyUpdater, BehaviorConfig
	  wander/             ← Goal, Action, BehaviorConfig
	objects/
	  food/               ← GdPAIObjectData + SpatialAction
	  fruit_tree/         ← GdPAIObjectData + SpatialAction with cooldown state

  demo_2d/                ← everything needed to run the 2D demo
	scenes/               ← open these in Godot
	assets/
	  agent/              ← reusable agent prefab + nav/animation scripts
	  world/              ← tileset, scenery, interactable object prefabs
	  ui/                 ← HUD scripts (FPS counter, agent debug label)
	configs/              ← GdPAIAgentConfig .tres resources
```

---

## Concept Map

| Folder | GdPAI class demonstrated |
|---|---|
| `shared/behaviors/hunger/` | `Goal`, `PropertyUpdater`, `GdPAIBehaviorConfig` |
| `shared/behaviors/wander/` | `Goal`, `NavigatingAction`, `GdPAIBehaviorConfig` |
| `shared/objects/food/` | `GdPAIObjectData`, `SpatialAction` |
| `shared/objects/fruit_tree/` | `GdPAIObjectData`, `SpatialAction`, validity checks with external state |

---

## The Demo Scenario

The 2D demo places agents in a tiled world containing **food items** and **fruit trees**.
Agents have a hunger value that decays over time. Their goals and priorities work as follows:

- **Hunger goal** — reward scales from 0 to 100 as hunger drops. At low hunger, this goal
  dominates and the agent will seek out food or shake a tree.
- **Wander goal** — fixed reward of 10. Kicks in when the agent is nearly full, or when no
  food-related actions are reachable.

The planner chains actions together: e.g. navigate to tree → shake tree → navigate to
fallen fruit → eat fruit. Cost is distance-based, so agents prefer the closest option.

---

## Using `shared/` in Your Own Project

The files in `shared/` are intentionally self-contained. To adapt them:

1. Copy the relevant folder into your project.
2. Rename the class (e.g. `HungerGoal` → `EnergyGoal`) and adjust the blackboard property
   name and reward formula for your domain.
3. Register your `BehaviorConfig` subclass in a `GdPAIAgentConfig` resource.
