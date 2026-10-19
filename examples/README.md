# GdPlanningAI Examples

## Running

1. From the GitHub source, build the Rust library for your platform once: `make -C addons/GdPlanningAI/rust build-release` (see the main README). A release zip already has it.
1. Open this project in Godot 4.x and enable the addon (Project → Project Settings → Plugins → "GdPlanningAI").
1. Open any `.tscn` at the `examples/` root and press **Play Scene** (F6). The project has no main scene, so F5 does nothing.

Read the scenes in this order; [docs/EXAMPLES.md](../docs/EXAMPLES.md) says what each one teaches and which scripts to open.

1. `hunger_basic_2d.tscn`: one agent balancing hunger and wandering.
1. `hunger_multi_agent_2d.tscn`, `hunger_stress_test_2d.tscn`: independent agents sharing food; the stress test scales the count.
1. `campfire_2d.tscn`: two goals (fire and hunger) and a cook-and-eat chain.
1. `campfire_3d.tscn`: the same logic in 3D.
1. `campfire_large_3d.tscn`: six agents, each pair tending its own fire.
1. `agent_showcase_3d.tscn`: skins, animation and carried props; planning is off.

## Layout

- `behaviors/`: goals, self actions, property updaters and behavior configs.
- `objects/`: object data and the actions each object offers.
- `shared/`: spawners and debug UI.
- `configs/`: agent config resources.
- `source_2d/`, `source_3d/`: prefabs, nav controllers and presentation.

Everything outside `source_2d/` and `source_3d/` is dimension-agnostic.

## Reusing a Behavior

1. Copy the folder (e.g. `behaviors/hunger/`) into your project.
1. Rename the classes and adjust the blackboard property and reward for your domain.
1. Add your behavior config to a `GdPAIAgentConfig`.
