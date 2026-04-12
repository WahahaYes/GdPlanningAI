# GdPlanningAI Examples

---

## Setup

1. Open this project in Godot 4.x
2. Enable the GdPlanningAI addon if not already enabled:
   - Project → Project Settings → Plugins
   - Enable "GdPlanningAI"
3. Navigate to `examples/` in the FileSystem dock - **demo scenes are at the top level!**

---

## Running Examples

### 2D Demos

**Navigate to:** `examples/` (root level)

- **`hunger_basic_2d.tscn`** — Basic single-agent hunger management
  - One agent foraging for food and shaking fruit trees
  - Good starting point to understand core concepts
  
- **`hunger_multi_agent_2d.tscn`** — Multiple agents competing for resources
  - Several agents with independent planning
  - Demonstrates resource contention and goal prioritization
  
- **`hunger_stress_test_2d.tscn`** — Performance stress test
  - Many agents (configurable spawner)
  - Tests planning engine performance under load

**To run:** Open any `.tscn` file at `examples/` root and press **Play Scene** (F6)

---

## Folder Organization

**Demo scenes** are at the root for easy access (`hunger_basic_2d.tscn`, etc.)

**Dimension-agnostic code:**
- `behaviors/` - Goals, PropertyUpdaters, BehaviorConfigs
- `objects/` - ObjectData + Actions  
- `shared/` - Utility scripts
- `configs/` - Agent configuration resources

**Dimension-specific source files:**
- `source_2d/` - 2D prefabs and assets
- `source_3d/` - 3D prefabs and assets

---

## Concept Map

| Folder | GdPAI class demonstrated |
|---|---|
| `behaviors/hunger/` | `Goal`, `PropertyUpdater`, `GdPAIBehaviorConfig` |
| `behaviors/wander/` | `Goal`, `Action`, `GdPAIBehaviorConfig` |
| `objects/food/` | `GdPAIObjectData`, `SpatialAction` |
| `objects/fruit_tree/` | `GdPAIObjectData`, `SpatialAction`, validity checks with external state |

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

## Using Behaviors and Objects in Your Own Project

The files in `behaviors/` and `objects/` are dimension-agnostic and intentionally self-contained. To adapt them:

1. Copy the relevant folder (e.g., `behaviors/hunger/`) into your project.
2. Rename the class (e.g., `HungerGoal` → `EnergyGoal`) and adjust the blackboard property
   name and reward formula for your domain.
3. Register your `BehaviorConfig` subclass in a `GdPAIAgentConfig` resource.

The `source_2d/` folder contains 2D-specific prefabs and assets. The `shared/` folder contains dimension-agnostic utility scripts (UI helpers, spawners). The runnable demos at the root (`hunger_basic_2d.tscn`, etc.) reference these source files.