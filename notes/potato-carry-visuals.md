# Potato Raw/Cooked Prefabs + Carry Visuals (working plan)

Date: 2026-09-07. Status: DONE — prefabs built, 5/5 new tests pass, no regressions (6 failing tests identical on HEAD). No `campfire_3d.tscn` hookup yet.

## Why this shape

- `held_item` is a blackboard string (`""`, `"wood"`, `"potato"`, `"cooked_potato"`); `DigPotatoAction` queue-frees the world potato and `CookPotatoAction` only rewrites the string. Nothing visual exists for carried items.
- `GdPAIBlackboard` is a native RefCounted with no change signal (checked `rust/src/gdpai_blackboard.rs`), so the carry script polls each frame — same pattern as `campfire_runtime.gd` and `agent_debug_label_3d.gd`.
- Held visuals must be separate lightweight scenes: instancing the world `potato_3d.tscn` under the agent would drag in collision + `GdPAIObjectData` providers.

## Files

- `examples/source_3d/prefabs/potato_3d.tscn` (kept uid/wiring): raw world potato reskinned in place — ellipsoid body + 3 eye specks, same collision/label/GdPAI nodes so the spawner reference is untouched.
- `examples/source_3d/prefabs/held_potato_raw_3d.tscn` (new, visual-only): raw skin.
- `examples/source_3d/prefabs/held_potato_cooked_3d.tscn` (new, visual-only): golden skin + slight emission + flesh split on top.
- `examples/source_3d/assets/agent/carry_visual_3d.gd` (new): socket script, `@export gdpai_agent` + `held_visuals` map (`potato`/`cooked_potato`, empty default fills in `_ready`), hover bob + slow spin. Unknown ids (e.g. `wood`) show nothing.
- `examples/source_3d/prefabs/agent_3d.tscn`: `+CarrySocket` (Node3D y=1.6) with the script, `gdpai_agent = ../GdPAIAgent`. Rig-independent hover; a physical hand-hold later only needs a second socket.
- `test/unit/test_carry_visual_3d.gd` (new): swap/clear assertions on a standalone agent (ON_DEMAND, scheduler drain copied from the campfire smoke test).

## Verify

- Python structural check on edited `.tscn` (ext_resource ids resolve).
- Headless resource probe: emission props survive save for per-pixel shading.
- `godot --headless -s addons/gut/gut_cmdln.gd -gtest=<carry test> -gexit`, then full `make test-godot`, `make lint-style`, `make format-godot`.

## Follow-ups (not this pass)

- `wood` held visual (pairs with the wood-pile KayKit pass); hand-hold socket once a character rig lands; 2D raw/cooked parity; spawner keeps spawning raw only (cooked exists purely as held state).
