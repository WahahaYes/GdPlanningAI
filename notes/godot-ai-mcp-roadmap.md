# Godot AI MCP Integration — Research + Roadmap

**Date**: 2026-09-07 **Context**: Hook `hi-godot/godot-ai` into GdPlanningAI via `godotenv` `addons.jsonc`, expose to OpenCode harness, dry-run benchmark = fix `campfire_3d.tscn` **Status**: v3.2.5 installed, Phase 2 next **Scope**: Personal experiment — no README/docs changes, OpenCode settings stay local + gitignored

## Goal

1. Install Godot AI addon reproducibly via `godotenv` (pinned checkout in `addons.jsonc`).
1. Expose it to this agent harness (OpenCode `mcp` config, project-scoped).
1. Prove value with golden test: use MCP tools to make sensible edits to `examples/campfire_3d.tscn`, which currently lacks a usable character controller / camera / navmesh, while `examples/campfire_2d.tscn` mostly works.

## Research

### 1. godot-ai upstream

- Repo: https://github.com/hi-godot/godot-ai — **46 tools / 120+ ops** (`docs/TOOLS.md`): scene hierarchy, node create/set, batch_execute, camera_manage, material/resource/csg/gridmap, script_create/attach/patch, signal/input_map/project_manage, project_run + logs_read + editor_screenshot + game_manage, test_run.
- Install location: `your-project/addons/godot_ai/` with `plugin.cfg` inside. Enable via Project Settings → Plugins → Godot AI. Dock shows MCP status + per-client Configure buttons.
- Requirements: Godot **4.7+** (we have 4.7.1 mono — OK), `uv`/`uvx` on PATH (we have uvx 0.11.32 — OK), MCP client, editor must be **running** (server is useless headless).
- Transport (v4, `main`): `MCP client → godot-ai attach (stdio) → Python server (auth HTTP 127.0.0.1:8000) → editor plugin (auth WS 127.0.0.1:9500)`. Both hops use rotating capabilities. Bare `http://127.0.0.1:8000/mcp` entry **cannot auth** — unsupported. Canonical launch is dock-generated `uvx --isolated ... --from godot-ai==<ver> godot-ai attach --port 8000 --ws-port 9500 [...]`.
- Version trap: `main` = unpublished **v4.0.0** candidate (`pyproject.toml` + `plugin.cfg` both 4.0.0). Docs (`v4-migration.md`, `packaging-distribution.md`) say publication is fail-closed pending qualification — "Do not install an unpublished candidate into a real project". Latest **published release is v3.2.5** (2026-09-03). `main` opencode descriptor already contains attach-migration (#838); `v3.2.5` tag serves the same `opencode.gd` content when fetched — treat version pin as open decision, default to `v3.2.5` for stability, document why if we pick `main`.
- Sources: `README.md`, `docs/plugin-architecture.md`, `docs/client-configuration.md`, `docs/TOOLS.md`, `docs/testing.md`, `plugin/addons/godot_ai/clients/opencode.gd`, `src/godot_ai/attach/main.py`.

### 2. OpenCode harness (this agent)

- OpenCode MCP docs: https://opencode.ai/docs/mcp-servers/ — servers live under top-level `mcp` key in `opencode.json`/`opencode.jsonc`. `type: local` = `command: [...]` argv array + optional `environment` (not `env`), `cwd`, `timeout`, `enabled`. `type: remote` = `url` + `headers`.
- godot-ai `clients/opencode.gd` descriptor: `config_type=json`, key path `["mcp"]`, file `~/.config/opencode/opencode.json` (all platforms, override `$OPENCODE_CONFIG`), `command_shape=COMMAND_ARRAY`, `type=local`, env under `environment`, user keys preserved: `enabled, timeout, environment, cwd`. Stale `url/headers` scrubbed on migrate; `command_supports_url_fallback=true`.
- Local state verified 2026-09-07: global `~/.config/opencode/opencode.json` only has `model`+`small_model`; `~/.config/opencode/opencode.jsonc` is empty stub; **no project `opencode.json` exists yet**; `opencode` binary at `/home/ethan/.opencode/bin/opencode`. Project config merges over global — prefer per-project file so the change is committable.
- Reconstructed canonical entry (confirm exact ports/version from dock Configure — authoritative):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "godot-ai": {
      "type": "local",
      "command": ["uvx", "--isolated", "--no-config", "--no-env-file", "--no-sources", "--no-build", "--index", "https://pypi.org/simple", "--default-index", "https://pypi.org/simple", "--find-links", "https://pypi.org/simple/godot-ai/", "--index-strategy", "first-index", "--keyring-provider", "disabled", "--link-mode", "copy", "--from", "godot-ai==3.2.5", "godot-ai", "attach", "--port", "8000", "--ws-port", "9500"],
      "enabled": true
    }
  }
}
```

- Manage via `opencode mcp list / mcp debug godot-ai`. Gate noise with `"tools": {"godot-ai*": false}` globally + per-agent allow if needed.

### 3. godotenv addon wiring

- Current `addons.jsonc`: `gut` (remote `v9.6.0`, `subfolder: addons/gut`) + `GdTimeMachine` (symlink `../GdTimeMachine`). Defaults `path: addons/`, `cache: .addons/`. Key rule: dict key must equal installed folder name. GodotEnv 2.17.0 verified; `make addons-install` wraps `godotenv addons install`.
- Proposed entry (key must be `godot_ai`):

```jsonc
"godot_ai": {
  "url": "https://github.com/hi-godot/godot-ai",
  "checkout": "v3.2.5",
  "subfolder": "plugin/addons/godot_ai"
}
```

- Caveats: godotenv git-checkout bypasses the release's signed-manifest verification + dock updater (acceptable for dev, note it). After install: add `addons/godot_ai/` to `.gitignore`, enable plugin (`project.godot` `[editor_plugins]` or UI), run dock Configure for opencode. Upstream layout `plugin/addons/godot_ai/` → `addons/godot_ai/` mapping must be smoke-tested — godotenv copies `subfolder`, not repo root.

### 4. Benchmark gap — campfire_2d vs campfire_3d

Full audit via explore agent; key files `examples/campfire_3d.tscn:19-60`, `examples/source_3d/prefabs/agent_3d.tscn:19-51`, `examples/source_3d/assets/agent/nav_controller_3d.gd:16-25`.

| Area | 2D (works) | 3D (broken/missing) | |---|---|---| | Nav | `NavigationRegion2D` + baked polygon `campfire_2d.tscn:14-16` | Empty `NavigationMesh` sub-resource `campfire_3d.tscn:14-15`, never baked → `GoToAction` no-ops | | Ground | `ColorRect` visual-only (fine top-down) | `CSGBox3D` visual-only, `use_collision=false`, no `StaticBody3D` `campfire_3d.tscn:48-50` | | Camera | `Camera2D` works zero-config `campfire_2d.tscn:56` | `Camera3D` fixed transform, **no `current=true`**, no controller script `campfire_3d.tscn:56-57`; no `[input]` map in `project.godot` | | Controller | `nav_controller.gd` complete for zero-g top-down | `nav_controller_3d.gd:20-25` overwrites `velocity`, no gravity/floor_snap, floats/clips | | Visuals | `agent.tscn` + `agent_animator.gd` Run/Idle | Static blue `CSGBox3D`, dead `BoxMesh_1` `agent_3d.tscn:13-14`, no animator | | Lighting | None needed | Only `DirectionalLight3D`, no `WorldEnvironment` → flat/black PBR | | Prefab bug | UIDs pinned | `campfire_3d.tscn:8` expects `uid://fnsnn6jmne4e` for `potato_3d.tscn` but file header is `uid://ea2n8p3r6t9u5` — UID mismatch; all 3D `ext_resource`s lack `uid:` | | Bounds | Nav polygon bounds AI | Open 20m slab, `WanderAction` can wander to infinity |

Not broken: shared `campfire_agent_config.tres`, `GdPAILocationData` 2D/3D dual, `GoToAction`/`WanderAction`/`potato_spawner_3d.gd`/`agent_debug_label_3d.gd` are 3D-aware.

## Roadmap

### Phase 0 — Decide + record (this note, done)

- [x] Research godot-ai transport/config, godotenv schema, opencode MCP shape, 3D gap audit.
- [x] Pin decided: `v3.2.5`. Scope: personal experiment, no README changes, `opencode.json`/`opencode.jsonc`/`.opencode/` gitignored local artifact.

### Phase 1 — Reproducible addon install (done 2026-09-07)

- [x] Add `godot_ai` entry to `addons.jsonc` (pin `v3.2.5` — decided).
- [x] `godotenv addons install` → `addons/godot_ai/plugin.cfg` version 3.2.5 verified, `plugin.gd` present. Note: one transient `git checkout v3.2.5` failure in `.addons/godot_ai` (exit 1) cleared on retry — tag exists upstream, manual `git -C .addons/godot_ai checkout v3.2.5` also OK. Likely race between background + foreground install sharing the cache. If it recurs: `rm -rf .addons/godot_ai` then retry.
- [x] `.gitignore`: `addons/godot_ai/` + `opencode.json`, `opencode.jsonc`, `.opencode/` (local experiment, not committed). Confirm `.addons/` cache ignored.
- [ ] Do NOT hand-edit `project.godot` plugins yet — enable via editor UI once; keep diff local unless team adopts.

### Phase 2 — Editor + MCP handshake (done 2026-09-07)

- [x] Editor open, Godot AI 3.2.5 enabled, server uvx `godot-ai==3.2.5` :8000/:9500, session `gdplanningai@fcbf`, `campfire_3d.tscn` open.
- [x] `scene_get_hierarchy` 53 nodes baseline; `camera_manage list`; `filesystem_manage search` PackedScene in `examples/source_3d` → 4 files.
- [x] `test_run` N/A — repo uses `test/` + GUT (`make test-godot`), not `res://tests/`; godot-ai `test_run` reports "No test suites found". No regression path via MCP for this repo layout.

### Phase 3 — Golden benchmark: fix campfire_3d via MCP (done 2026-09-07, editor session `gdplanningai@fcbf`)

MCP-verified writes (each confirmed via `node_get_properties` + `scene_save`):

1. [x] Baseline `scene_get_hierarchy` (53 → 62 nodes after edits).
1. [x] Camera `current=true` (`false→true`, undoable). `cinematic` screenshot now resolves via `/Campfire3D/Camera3D` (640px ok).
1. [x] Ground `use_collision=true` (`false→true`). Minimal fix; full StaticBody3D wrap deferred (CSG collision suffices for demo).
1. [ ] Navmesh bake — NO tool in catalog (`search navigation navmesh` → []). Recorded limitation; `GoToAction` still unproven on 3D nav.
1. [x] `nav_controller_3d.gd` gravity + XZ-flatten patch via `script_patch` (`diagnostics: []`, `reloaded: true`).
1. [x] `WorldEnvironment` + default `Environment`/ProceduralSky via `node_create` + `resource_manage environment_create`.
1. [partial] Potato UID `fnsnn6jmne4e→ea2n8p3r6t9u5` fixed via local text edit — MCP has no UID round-trip tool (`scene_save` preserved the mismatch). Dead `BoxMesh_1` in `agent_3d.tscn` left; no sub-resource prune tool.
1. [x] 4 boundary walls (`WallN/S/E/W` StaticBody3D + BoxShape3D via inline `__class__`, positioned ±10) + `PotatoSpawner.spawn_area_size` set explicit (was already 16,1,16 default; now serialized).
1. [x] `project_run current` → `live`, `helper_live:true`, `recent_errors:[]`, `project_manage stop` clean; `logs_read editor` 0 entries.

Pass criteria: 7/9 via MCP alone (camera, ground, controller, lighting, walls, spawner, run/verify). Fail/limitation recorded: navmesh bake + UID round-trip need non-MCP edits. No editor restart needed per edit.

### Phase 4 — Harden

- Pin working `godot-ai` checkout + uvx `--from` version in lockstep; document upgrade via dock **Update** (never extract over tree).
- `make` targets if warranted (`addons-install` already covers install; consider `mcp-debug` helper).
- Docstring/style: GDScript edits via MCP must still pass `make format-godot`, `make lint-style`, `make test-godot`.

## Risks / Open Questions

1. **v3 vs v4**: v4 unpublished — do we accept candidate risk for stdio attach, or stay on `v3.2.5` release? Dock-generated argv is authoritative; reconstruction above must be confirmed live.
1. **godotenv subfolder mapping**: verify `plugin/addons/godot_ai` → `addons/godot_ai` copies correctly; release ZIP layout may differ from git tree.
1. **Editor-bound**: no headless MCP — CI can't use it; benchmark requires interactive editor + `uvx` pre-warm (first spawn builds ~67 pkgs, can exceed MCP timeout).
1. **Context cost**: 46 tools is heavy — gate with `tools: {"godot-ai*": false}` globally + per-agent allow.
1. **Scope**: remote `type: remote http://127.0.0.1:8000/mcp` entries seen in older guides do NOT auth on v4 — use dock's `attach` argv.

## Next Action

Both campfire scenes hunger-mirrored and verified live. Remaining: navmesh bake (no MCP tool), UID pinning on 3D prefabs, animator, orbit input. Long-window 2D movement proof still open (needs fuel\<60 or high hunger; nav intact by construction).

## Follow-up — Full Hierarchy Cleanup, Both Scenes (2026-09-07)

- Rule applied (from hunger: `hunger_basic_2d.tscn:21`, `scenery.tscn:185`): root holds world-logic + actor(s) only; everything static nests under one container.
- 3D root now `GdPAIWorldNode + Agent1 + Terrain`; Terrain holds `NavigationRegion3D` (Ground, 4 walls), `Campfire`, `WoodPile×4`, `PotatoSpawner`, `DirectionalLight3D`, `WorldEnvironment`, `CameraRig`. Safe: spawners parent to `get_parent()` (`potato_spawner_3d.gd:47`) with parent-local coords, Terrain identity ⇒ same globals; `camera_follow_3d.gd` `_ready` now resolves via `current_scene` so rig placement is free. Verified: cinematic via moved rig, run live, rig tracking (y≈agent+1.2).
- 2D root now `GdPAIWorldNode + Agent1 + Terrain`; Terrain holds `Ground`, `NavigationRegion2D` (baked polygon intact, region was at origin), `Camera2D`, `Campfire`, `WoodPile×4`, `PotatoSpawner`. Verified: run live, no errors, hunger ticking (6.6 after ~5s).
- MCP lesson: `game_eval` budget is 8s — a 15s timer-await aborts (EVAL_HUNG) and can park the game at a debugger break; recover with `project_manage(stop)`. The 2 break errors were eval artifacts, not project errors (buffer clean after stop).

## Incident — MCP Reparent Materialized Instance Children (2026-09-07, fixed)

- Symptom (2D): agent invisible, campfire label frozen at 100%.
- Two stacked causes found via live evals: (a) draw order — `Agent1` sat before `Terrain`, so the 800×600 `Ground` rect painted over it (fixed: `Agent1` moved last, matching hunger's Scenery-then-Agent order); (b) `reparent_node` on instance roots materialized prefab children as override blocks (+200 lines 2D, +160 3D), and the runtime bound to orphan duplicates (`same=false`, ref label ticking 71% while tree label frozen, fuel decaying underneath at 3.0/s).
- Fix: `/tmp/opencode/prune2.py` (backups in `/tmp/opencode/*.bak.tscn`) deleted descendant-of-instance blocks + newly-orphaned resources, guarded by asserts (scene root never an instance root; deleted unique_ids absent from HEAD; pruned resources all new). First version of the script had a root-poisoning bug caught by its own dry run — rewritten explicit.
- Verified post-fix: 2D `same=true`, label 91%→67%, hunger rising; 3D `same=true`, 79%→55%; both runs live, no errors. Compact counts: 2D 52 nodes, 3D 65 nodes total.
- Corrections to earlier claims: the 3D bake WAS on disk all along (4 verts/2 polys @y=0.75) — my `grep vertices=` missed `vertices =` spacing. Nothing baked was lost in surgery.
- Standing rule: never MCP-reparent instance roots; move plain nodes only, or edit `.tscn` text + `force_reload`.
- Labels: `billboard=1` on all four 3D prefabs (verified live). `fixed_size=true` tried and reverted same session — it renders glyphs at constant screen size and the labels filled the screen. Billboard alone preserves world scale. Note: this agent cannot view screenshot pixels, only capture metadata (size, camera path) — visual acceptance stays with the user.

## Follow-up — Orbit Camera: Drag, Limits, Reset Key (2026-09-07)

- `camera_follow_3d.gd` gained orbit: right/middle-drag rotates the SpringArm (`orbit_pixels`, yaw free, pitch clamped −75°..−15°), wheel zooms (length 4..14), `R` resets pose+zoom. No InputMap changes — raw `_unhandled_input`, so `project.godot` untouched. `lint_style` clean.
- Verified live, all exact: 200px drag → Δyaw −1.0; pitch slams clamp at −1.309/−0.262; zoom slams at 14/4; `reset_view` restores (−0.9, −0.78, 8). Synthetic `R` keypress via `game_manage input_key` reset a perturbed (−1.28) arm to defaults — full input path proven.
- Caveats: synthetic mouse-motion via the debugger bridge carries no button mask, so drag itself was proven at unit level (direct calls), not through injected motion — needs a human with a mouse for final acceptance. Helper-eval flakiness (250ms probe vs planner bursts) means orbit tests must run in small evals on a fresh launch; also learned to assert `scene_file_path` in-eval after the Campfire2D-vs-3D mixup parked a run at a break (eval errors, not game errors).

## Follow-up — Proper 3D Controller + Follow Camera (2026-09-07)

- `nav_controller_3d.gd` upgraded via `script_patch` (clean diagnostics): `acceleration=12`, `turn_speed=10`, `gravity=9.8`, `arrival_threshold=0.2`, XZ smoothing via `lerpf`, yaw facing via `lerp_angle(atan2)`, gravity preserved. `lint_style` clean.
- `camera_follow_3d.gd` created (`examples/source_3d/assets/camera/`): `target`, `height=1.2`, `follow_speed=5`, `_ready` auto-resolves `../Agent1` (MCP `node_set_property` rejects Node-to-Node assignment with VALUE_OUT_OF_RANGE, so explicit target link stays null in favor of auto-resolve).
- Rig built via MCP: `CameraRig` (Node3D at -3,1.7,-3) → `SpringArm3D` (rot -0.9,-0.78, length 8) → reparented `Camera3D` (local 0,0,0, `current:true` preserved). `scene_save` ok.
- Verify: `cinematic` resolves via `/Campfire3D/CameraRig/SpringArm3D/Camera3D` (640px), `project_run current` live, no errors, stop clean.

## Follow-up — Terrain Cleanup + Bake Rule Correction (2026-09-07)

- Docs claim (`NavigationMesh.geometry_source_geometry_mode` default ROOT_NODE_CHILDREN) read as whole-scene bake was wrong in practice: user moved `Ground` under the region and bake succeeded. Authoritative rule for this scene: **bake sources must live under `NavigationRegion3D`**.
- Mirrored hunger's `Scenery` semantics (`hunger_basic_2d.tscn:21`, `scenery.tscn:185-264` — static world container, actors at root): new `Terrain` (Node3D, identity) parents `NavigationRegion3D`, which parents `Ground` + `WallN/S/E/W`. Gameplay objects, agent, camera rig stay at root (as in `campfire_2d.tscn`).
- All moves via MCP (`node_create` + `reparent_node`; note `batch_execute` wants `reparent_node`, not `reparent`). Identity parents ⇒ locals == globals, baked mesh stays valid, no rebake. Verified: top-level 13 nodes, cinematic 640px, run live, no errors.

## Follow-up — Green Suite: 2D Paths + Half-Fuel Start (2026-09-07)

- 2D smoke `_setup` now reads `Terrain/Campfire` (post-cleanup path); full 5-action cook chain passes.
- `CampfireObject.current_fuel` is an `@export` defaulting to 50.0, so both scenes start half-full (desired 60 keeps Maintain-Fire live from the start). Fuel asserts use ±1.0 tolerance for setup burn-down.
- Suite is 80/80 green two runs in a row (was 6 failing on HEAD). The clone/proxy failures were collateral: crashed smoke setups leaked scheduler state into later tests in the shared GUT process — one root cause, not three.

## Follow-up — Campfire 3D Hookup (2026-09-07)

- All prefab work applied: skinned Knight agent, particle campfire, log-stack piles, reskinned potato, shared day sky. Scene-side changes are two heights only — `Agent1` y 0.5→0.25 (collider realignment), spawner y 0.5→0.25 (new potato origin at ground). Campfire/pile instances already sat at 0.25.
- `test/integration/test_campfire_3d_scene.gd` (7 tests): agent rest height, skin + socket present, fire particles, 4× LogStack, half-full start, spawner keeps 5 seated potatoes, pile push-out, campfire avoidance. All pass.
- Layout: piles out to (±7, ±3.5) (~7.8m from fire); `PotatoSpawner3D` rejects points inside 5m of any `CampfireObject` (positions gathered at ready, best-of-16 fallback). Pinned by push-out + avoidance tests.

## Follow-up — Showcase Idle Pauses + Walk-Loop Fix (2026-09-07)

- Walk "not looping" root cause: every Rig_Medium clip imports as `LOOP_NONE`, so `Walking_A` played once (1.07s) then froze mid-stride — idle would have done the same. `agent_skin_3d.gd` now forces `LOOP_LINEAR` on its configured idle/walk pair; one-shots keep imported mode.
- Driver idles: arrival/retarget rolls `idle_chance` (default 0.35, 1.5–4s); pausing parks `target_position` under the body — without that the controller chases the old target straight through the pause (caught by the new test, then fixed).
- Stuck-idle bug: the idling branch decremented a local copy of the countdown without writing it back, so pauses never ended. Resume test (chance flipped 1.0→0.0 mid-run) pins walk→idle→walk.
- Showcase hides agent `Label3D`s via one-line `visible = false` descendant overrides (property-only — no reparent, no materialization risk).
- Collider audit (headless shape measurement): campfire sunk 0.15 and potato sunk 0.3 — both CollisionShapes now rest at local 0 (campfire y=0.15, potato r=0.25 at y=0.2). Pile and agent were already correct. Pinned by `test/unit/test_prefab_colliders_3d.gd` (bottom ≈ 0 for all four).
- Sky is now shared art: `examples/source_3d/assets/environment/day_sky_3d.tres` (brightened daytime gradient), referenced by both 3D scenes — not a config.
- First decision always walks so agents leave spawn; showcase test forces chance 1.0 to pin the walk→idle cycle deterministically.

## Follow-up — Showcase Scene + Collider Fix (2026-09-07)

- Suspected misaligned colliders confirmed by geometry: the collision cylinder (h=1.0) sat centered on the body origin while character feet are at local y=0, so the visual sank half a meter relative to physics. Fix: `CollisionShape3D` y-offset 0.5 (bottom at feet) in `agent_3d.tscn`. Showcase asserts rest height 0.25 == ground top.
- `campfire_3d.tscn` still places `Agent1` at y=0.5 — now floats 0.25 above ground. Flagged for the campfire-hookup pass, deliberately untouched here.
- `examples/agent_showcase_3d.tscn`: 3 prefab agents (Knight/Barbarian/Mage × potato/wood/empty) driven by `assets/debug/showcase_driver_3d.gd` (skin + held-item assignment by index, ON_DEMAND parking, wander retargets). No per-agent override blocks — discovery via `find_children GdPAIAgent`. 6 integration tests pin carry states, skins, rest height, wander targets, rig tracking, and camera aim.
- Fixed-camera lesson: hand-written `Transform3D(...)` bases transpose silently — my oblique matrix aimed at the sky. Rule: author static Node3D poses with `position` + `rotation` euler, never raw bases. The showcase now rides the campfire orbit rig (right-drag orbit, wheel zoom, R reset) following AgentWood instead.

## Follow-up — Agent Skin: Knight + Retargeted Rig_Medium Clips (2026-09-07)

- Skin interface: `assets/agent/agent_skin_3d.gd` on the skin-scene root (which instances one Adventurers `.glb`); shared static clip cache from both Rig_Medium packs copied into an `AnimationPlayer` child; velocity poll drives Idle/Walk. `skins/knight_3d.tscn` is 6 lines — new skins just instance another `.glb`.
- Gotcha (text-edited `.tscn`): a NodePath export override on an instance node is silently dropped unless the node header declares `node_paths=PackedStringArray(...)`. `Skin` showed `entity=null` until the attribute was added — same attribute every hand-written GdPAI node already carries. Structural grep for the fix: every `= NodePath(` assignment must have its property listed in the node's `node_paths`.
- `agent_3d.tscn`: blue CSG placeholder (+ dead mesh/material) replaced by `Skin`; label y→1.75, socket y→1.95 for the 0.6-scale Knight (~1.53 m). Physics/nav untouched.

## Live Observation (2026-09-07, editor closed after manual run)

- User: agent doesn't move; food visibly attaches but never changes state/disappears.
- Consistent with empty navmesh: `GoToAction.perform_action` (`goto_action.gd:192-208`) returns SUCCESS instantly when `is_navigation_finished()`, so Dig/Pickup execute remotely (`pickup_action.gd:95-96` sets `held_item` + `queue_free`, `dig_potato_action.gd:59-60` same) while the body never moves. Eat then needs 1.5s RUNNING plus a map hit (`eat_held_food_action.gd:105-125`, raw `potato` not edible — only `cooked_potato`/`banana` per `hunger_behavior_config.gd:14`), so a stuck/looping plan shows held food with no hunger drop.
- Separately captured pre-fix: `Hunger` planner returning `success=false, Branches: 1` every ~1s while `Maintain Fire` plan `[Go To, Pick Up Wood, Go To, Add Fuel]` executes — follow up post-navmesh whether Hunger failure was starvation-by-no-path or a real planning gap.
