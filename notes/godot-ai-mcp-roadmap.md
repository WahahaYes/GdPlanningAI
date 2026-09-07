# Godot AI MCP Integration — Research + Roadmap

**Date**: 2026-09-07 **Context**: Hook `hi-godot/godot-ai` into GdPlanningAI via `godotenv` `addons.jsonc`, expose to OpenCode harness, dry-run benchmark = fix `campfire_3d.tscn` **Status**: Research complete, installing v3.2.5 **Scope**: Personal experiment — no README/docs changes, OpenCode settings stay local + gitignored

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

### Phase 1 — Reproducible addon install (dry, no editor yet)

1. Add `godot_ai` entry to `addons.jsonc` (pin `v3.2.5` — decided).
1. `make addons-install` / `godotenv addons install`, verify `addons/godot_ai/plugin.cfg` exists, `plugin.gd` present.
1. `.gitignore`: `addons/godot_ai/` + `opencode.json`, `opencode.jsonc`, `.opencode/` (local experiment, not committed). Confirm `.addons/` cache ignored.
1. Do NOT hand-edit `project.godot` plugins yet — enable via editor UI once; keep diff local unless team adopts.

### Phase 2 — Editor + MCP handshake

1. `make launch-editor`, enable Godot AI in Plugins, confirm dock status (server :8000/:9500).
1. Dock **Configure** for OpenCode (or **Configure all**). Capture canonical `command` argv it generates — replace reconstruction above.
1. Create local (gitignored) project `opencode.json` with `mcp.godot-ai` entry. Keep global `~/.config/opencode/opencode.json` untouched except via dock. Do not commit.
1. `opencode mcp list` + `opencode mcp debug godot-ai` → expect connected. `GODOT_AI_DISABLE_TELEMETRY=true` if team opts out.
1. Smoke prompts (editor open): "Show current scene hierarchy", "Search project for PackedScene in examples/source_3d", "Run scene test suite".

### Phase 3 — Golden benchmark: fix campfire_3d via MCP

Sensible MCP edit sequence to attempt (each via `batch_execute`/`node_*`/`resource_manage` + `scene_save`, verify with `project_run` + `logs_read` + `editor_screenshot`):

1. `scene_open campfire_3d` → `scene_get_hierarchy` baseline.
1. Camera stopgap: `node_set_property Camera3D current=true`; then proper rig (`SpringArm3D` + follow/orbit script) + `[input]` actions if manual control desired.
1. Ground collision: wrap/replace `Ground` with `StaticBody3D + CollisionShape3D(Box 20,0.5,20) + MeshInstance3D`, top at y=0; re-seat campfire/wood/agent Y.
1. Bake navmesh for 20×20 slab (or assign baked `.navmesh`), add bake exclusions around campfire logs.
1. Gravity in `nav_controller_3d.gd` (`velocity.y -= 9.8*delta`, flatten XZ, `move_and_slide`), set `floor_snap_length`.
1. `WorldEnvironment` + ambient/sky; verify `DirectionalLight3D` rotation.
1. Fix potato UID mismatch + pin missing `uid:` on 3D `ext_resource`s; remove dead `BoxMesh_1` or wire it.
1. Add boundary walls; set `PotatoSpawner.spawn_area_size` explicitly.
1. `project_run campfire_3d` → agent debug label shows Goal/Action cycling, `editor_screenshot(game)` shows lit agent + campfire, no SCRIPT ERROR in `logs_read`.

Pass criteria: MCP alone can perform 1–9 without hand-editing `.tscn`/`.gd` in a text editor. Fail criteria worth recording: MCP can't bake navmesh, can't set `current`, can't round-trip UIDs, or needs editor restart per edit.

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

Pin decided (`v3.2.5`). Phase 1 install running (`godotenv addons install` in background — timed out at 120s foreground, retrying); verify `addons/godot_ai/plugin.cfg` then Phase 2.
