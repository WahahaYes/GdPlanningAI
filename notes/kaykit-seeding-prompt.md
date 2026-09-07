# Seeding Prompt: KayKit 3D Assets + Campfire 3D Scene

Paste into a fresh session working on the GdPlanningAI 3D demos. Assumes the repo at `/home/ethan/repos/GdPlanningAI`, Godot 4.7.1 mono via godotenv, OpenCode harness. Living log: `notes/godot-ai-mcp-roadmap.md`.

______________________________________________________________________

You are working on GdPlanningAI's 3D example scene (`examples/campfire_3d.tscn`) and its vendored 3D assets. Context:

## Toolchain (must know)

- Godot version and addons are managed by **godotenv**: `addons.jsonc` pins `gut` and `godot_ai` (**v3.2.5**, do not upgrade casually). Install with `make addons-install` / `godotenv addons install`.
- **godot-ai MCP** is the editor bridge. It only works while the Godot editor is open with the Godot AI plugin enabled (session appears via `session_manage(op=list)`). In this harness call its tools through `default.execute` as `tools["godot-ai"].<tool>({...})`, and use bare `search({...})` inside the same runtime to discover tool paths. OpenCode MCP config is local-only and gitignored.
- `project_run(mode="current")` presses Play in-editor and reports `game_status` (`live`/`not_live`/`break`/…), `helper_live`, and `recent_errors`. `game_eval` aborts after **8s** — keep awaits under ~6s or the run can park at a debugger break (recover with `project_manage(op=stop)`). Assert `scene_file_path` inside evals; the wrong scene has bitten before. The helper-eval channel flaps under planner load — small evals on a fresh launch work best, and `game_manage` (debugger bridge) often answers when evals don't.

## Vendored assets (the pattern)

- Third-party packs live at `examples/source_{2d,3d}/assets/vendored/<Pack>/` with original internal layout preserved. Index + license copies: `examples/VENDORED_ASSETS.md`, `examples/licenses/`.
- 2D: Tiny Swords by Pixel Frog, Free Pack only (no-redistribution terms — transcribed from the store page, no file ships in the pack).
- 3D: **KayKit Adventurers 2.0 FREE** (characters `.glb`, props `.gltf`+`.bin`, animations) + **KayKit ResourceBits 1.0 FREE** (props), glTF-only, both CC0 with `License.txt` in-pack. Regenerate the viewer with `python3 scripts/generate_kaykit_gallery.py`.
- Viewer scene: `examples/source_3d/viewer/kaykit_gallery.tscn` (146 instances on a grid; verified headless-clean).

## Campfire 3D scene setup (hard-won rules)

- Hierarchy mirrors hunger's Scenery pattern: root holds `GdPAIWorldNode` + `Agent1` + `Terrain` only. `Terrain` parents the `NavigationRegion3D` (Ground CSG + 4 wall StaticBodies underneath it), gameplay objects (Campfire, WoodPiles, PotatoSpawner), lights, and the CameraRig. Same shape in `campfire_2d.tscn`.
- **Navmesh bakes only pick up the region's subtree** (empirical — contradicts a naive docs reading). Keep bake sources nested under the region. Current mesh: one quad @y=0.75, nothing carved.
- Agent: `CharacterBody3D` + `NavigationAgent3D`, driven by `nav_controller_3d.gd` (smoothed XZ steering, yaw facing, gravity). Camera: `CameraRig` follow script + SpringArm — right/middle-drag orbits (pitch clamped −75°…−15°), wheel zooms (4…14m), `R` resets.
- All four 3D prefabs use `billboard = 1` debug labels. Do NOT add `fixed_size` — it blows glyphs up past the viewport (tried, reverted).
- **Never MCP-reparent prefab instance roots** — the move materializes instance children into override blocks and severs runtime NodePath bindings (frozen labels). Move plain nodes only; for instances, edit `.tscn` text + `scene_open(force_reload)`.
- Cosmetic, do not "fix" without asking: `campfire_3d.tscn` references the potato prefab by a stale UID (loads via path fallback).

## Known open items

- `Hunger` planner intermittently returns `success=false, Branches: 1` while Maintain-Fire plans execute — uninvestigated.
- 3D campfire has no carved obstacles, no animator, no orbit acceptance from a human with a mouse.
- KayKit assets are reserved, not yet wired into any demo.
