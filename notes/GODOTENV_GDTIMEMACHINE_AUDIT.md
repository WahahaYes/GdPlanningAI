# Audit: OBS Recording → GdTimeMachine via GodotEnv (colocated)

Date: 2026-09-03 Status: Draft — for planning

## 1. Executive Summary

- **This repo still ships its pre-split OBS recording stack.** `scripts/capture_obs.py` + `obs_controller.py` + `capture_all_showcase.sh` + `Makefile:record-obs` are the legacy path.
- **The replacement is already built next door at `../GdTimeMachine`.** Colocated sibling repo contains `addons/GdTimeMachine` with editor dock, 3 backends, and CLI `gdtime` for historical (worktree) capture. Parity as of 2026-08-26: OBS, Movie Maker, Screenshot + ffmpeg tier-2 + worktree/godot_resolve all implemented.
- **`RECORDING_INVENTORY.md` (2026-08-01) is still accurate as inventory** but its parity table is stale — GdTimeMachine has since closed the gaps.
- **GodotEnv already manages `addons/` in this repo** via `addons.jsonc` (currently only `gut`). The same mechanism supports a colocated local path/symlink for GdTimeMachine — no submodule, no copy-paste.
- **Recommended path: add a `symlink` (or `local`) entry for GdTimeMachine pointing at `../GdTimeMachine`** in `addons.jsonc`, run `godotenv addons install`, delete the legacy scripts after verification, and delegate batch capture to `gdtime run`.

## 2. Audit: What GdPlanningAI Currently Contains (Recording-Related)

### 2.1 Recent commits (the split)

```
925c360  recording via obs                 — .env.example, obs_controller.py rewrite, Makefile record targets
629d824  correct godot framerate            — --max-fps vs --fixed-fps
aa79de1  rm extra recording scripts         — deletes scripts/capture_scene.sh (298 lines)
3144bc2  setting up commit record           — capture_all_showcase.sh gains worktree/manifest logic
f8ac17a  build project correctly…           — proper Godot project build vs copying .godot
c78b88a  fix capture condition
bcfac9d  clean up recording scripts         — trims 406→198 lines across 3 files
e462990  record_addon_exploration branch    — readme rewrite, plan check-in (99b36cb, fef31ce)
```

Full diff 52de432^..bcfac9d: 3 new tracked files (`capture_obs.py`, `obs_controller.py`, `capture_all_showcase.sh`) + `.env.example` + Makefile + `.gitignore` updates + `notes/obs-recorder-addon/*` design docs.

Current HEAD `rust` branch is 1280×720-logo commits ahead; `record_addon_exploration` is the feature branch that staged the split.

### 2.2 Files on disk

| Path | Tracked | LOC / Size | Purpose | |---|---|---|---| | `scripts/capture_obs.py` | yes | ~300 lines | Main recorder: ensures OBS running, launches Godot scene (`godot --path … --scene res://… --max-fps`), drives `OBSController`, waits duration, stops | | `scripts/obs_controller.py` | yes | ~225 lines | WebSocket controller (obsws_python ReqClient), PipeWire RestoreToken persistence (`scripts/.obs_capture_token.json` + `~/.config/gdplanningai-obs/capture_token`), screen-source kinds | | `scripts/capture_all_showcase.sh` | yes | ~450 lines | Batch via git worktrees, manifest of 18 captures (4 acts), godotenv detection (`resolve_godot_bin` reads `project.godot:config/features`), cargo build, `.godot` regen | | `.env` | ignored | — | `OBS_HOST/PORT/PASSWORD` — sourced by Makefile | | `.env.example` | yes | 15 lines | Template for above | | `scripts/.obs_capture_token.json` | ignored | — | Wayland portal token | | `.worktrees/` | ignored | 10 worktrees | `01_before_pure_gdscript` (dc48fe5) → `10_final_campfire_3d` (a9bda00), plus `cargo-target` shared dir | | `media/captures/` | ignored | ~121 MB stray + acts | `act1_foundation`…`act4_polish` subdirs, standalone mp4s | | `Makefile:record-obs` | yes | — | `SCENE/DURATION/FPS/OUTPUT/FULLSCREEN` vars → `uv run scripts/capture_obs.py` | | `Makefile:record-obs-fullscreen` | yes | — | `FULLSCREEN=1` wrapper | | `notes/obs-recorder-addon/*` | yes | 5 files | `ARCHITECTURE.md`, `IMPLEMENTATION_PLAN.md`, `BRAINSTORM.md`, `ENHANCEMENT_CLI_COMPANION.md`, `RESEARCH.md` — **design birthplace; keep** | | `notes/RECORDING_INVENTORY.md` | yes | — | Inventory + cleanup checklist (see §7) | | `notes/YOUTUBE_SHOWCASE_COMMITS.md` | yes | — | Commit timeline (dc48fe5 … a9bda00, 4 acts) |

`.gitignore` lines: `addons/gut/` + `.addons/` (godotenv), `.env`, `.obs_capture_token.json`, `media/captures/`, `.worktrees/`.

### 2.3 Dependencies

- `obsws_python` (ReqClient), `python 3.13` via `uv`, `OBS WebSocket v5` (port 4455), `godot` CLI, `cargo` optional.

## 3. Audit: What GdTimeMachine Contains (Colocated `../GdTimeMachine`)

Repo at `/home/ethan/repos/GdTimeMachine` (10 commits ahead of GdPlanningAI's design docs). `addons/GdTimeMachine` layout:

```
addons/GdTimeMachine/
  plugin.cfg / plugin.gd
  backend/recorder_backend.gd (98 LOC abstract)
         /backend_obs.gd (973 LOC) — WebSocket, auto-launch/minimize, status narration
         /backend_movie_maker.gd (560 LOC) — EditorInterface movie_writer, 4GB AVI cap
         /backend_screenshot_capture.gd (796 LOC) — IN_PLACE, no restart
         /ffmpeg_convert.gd (561 LOC) — tier-2 MP4/WebM
  core/worktree.gd (206 LOC)   core/godot_resolve.gd (64)  core/movie_writer.gd (67)  core/build_runner.gd (59)
  cli/gdtime (shim) + cli/main.gd (751) + cli/record.gd + schema/batch_manifest.schema.json
  controller/recorder_controller.gd  config/* (composite/project_local/editor_settings stores, profiles.cfg)  ui/time_machine_dock.*
  vendor/obs_client.gd  autoload/graceful_stop.gd
```

CLI `gdtime` (headless `godot --headless -s cli/main.gd --`): `validate <manifest>` / `run [--dry-run --resume LABEL --keep-worktrees --no-git --force --build-timeout 600 --fail-fast --strict]` / `doctor [--verbose --fix]` / `list-commits`. Uses `godot --path <worktree> --write-movie` (Vulkan, no `--headless`), `godotenv`/`GODOT_BIN` resolution, `rm -rf .godot && godot --editor --headless --quit` regen.

### Parity vs legacy (updates RECORDING_INVENTORY § parity)

| Capability | Legacy in GdPlanningAI | GdTimeMachine now | |---|---|---| | OBS WebSocket capture | `capture_obs.py` + `obs_controller.py` | `backend_obs.gd` + `vendor/obs_client.gd` — GDScript-native, auto-launch/close, tray, narration | | Historical commit recording | `capture_all_showcase.sh` + `.worktrees/` | `cli/main.gd` + `core/worktree.gd` + `godot_resolve.gd` + `build_runner.gd` — manifest-driven, resume, strict | | Scene-based recording | `record-obs` make targets | Editor dock (bottom panel) + `Ctrl+Alt+R` + Command Palette + per-scene `profiles.cfg` | | Credential/config | `.env` + `.env.example` | `EditorSettings gd_time_machine/obs/*` + `gd_time_machine/recorder/*` + `gd_time_machine/ffmpeg/*` | | Movie Maker | — (Python launched Godot) | `backend_movie_maker.gd` + `core/movie_writer.gd` | | ffmpeg MP4/WebM | — | `ffmpeg_convert.gd` tier-2 (auto_convert, clean_frames) | | Formats | implicit mp4 | avi/ogv/png/jpg native + mp4/webm via ffmpeg (backend-aware dropdown) |

Result: **GdTimeMachine reaches and exceeds parity**; the only deferred items are dock batch UI, replay buffer, CI runner (see `../GdTimeMachine/notes/Deferred`).

## 4. GodotEnv Mechanics Relevant Here

GodotEnv (Chickensoft `dotnet tool install --global Chickensoft.GodotEnv`) does two jobs:

1. **Godot version management** (`godot install/use/pin`, `.godotrc`/`global.json`, `GODOT` env/symlink, `godotenv godot env target`).
1. **Addon management** (`godotenv addons install` reads `addons.jsonc` in CWD).

`addons.jsonc` schema (`https://chickensoft.games/schemas/addons.schema.json`):

```jsonc
{
  "$schema": "https://chickensoft.games/schemas/addons.schema.json",
  // "path": "addons",   // default
  // "cache": ".addons",  // default
  "addons": {
    "<install-dir-name>": {
      "url": "https://github.com/...  OR  ../relative/path  OR  /abs/path",
      "source": "remote" | "local" | "symlink" | "zip", // remote=default
      "checkout": "main",    // branch/tag/commit for git sources; ignored for symlink
      "subfolder": "addons/foo" // subdir inside source repo to copy; default "/"
    }
  }
}
```

- `cache: .addons` holds cloned/cached sources; installed result is copied/symlinked into `path` (default `addons/`).
- `.gitignore` should contain `addons/*` + `!.addons/.editorconfig` (generated by `godotenv addons init`) and `.addons/` itself. GdPlanningAI already ignores `addons/gut/` + `.addons/`.
- Verification: `godotenv addons install` is idempotent; it warns if installed files were hand-edited (temp git repo check).

For colocated dev we care about **local filesystem sources**:

- `source: "local"` — source must be a **git repo**; GodotEnv clones/copies from the filesystem path into `.addons` then copies `subfolder` into `addons/<name>`.
- `source: "symlink"` — creates a **symlink** in `addons/<name>` pointing at `url` (+ `subfolder`). Source need not be a git repo. Edits are live in both directions — ideal for developing GdTimeMachine alongside GdPlanningAI.
- `source: "remote"` with `url: "file://…"` is not the idiomatic colocated pattern; use `local`/`symlink`.

Both `local` and `symlink` accept relative paths (relative to project root where `addons.jsonc` lives). Upstream docs show `../my_addons/local_addon` and `/Users/me/...` examples.

GdPlanningAI today:

```jsonc
// addons.jsonc
{
  "addons": {
    "gut": { "url": "https://github.com/bitwes/Gut", "checkout": "v9.6.0", "subfolder": "addons/gut" }
  }
}
```

Invocation: `godotenv addons install`, `make addons-install`, `godotenv godot env get` etc. exist in both repos' Makefiles.

## 5. Migration Options

### Option A — `symlink` to colocated checkout (Recommended for dev)

```jsonc
{
  "addons": {
    "gut": { "url": "https://github.com/bitwes/Gut", "checkout": "v9.6.0", "subfolder": "addons/gut" },
    "GdTimeMachine": {
      "url": "../GdTimeMachine",
      "source": "symlink",
      "subfolder": "addons/GdTimeMachine"
    }
  }
}
```

- Behaviour: `addons/GdTimeMachine` becomes a symlink → `../GdTimeMachine/addons/GdTimeMachine`. No copy, no `checkout`.
- Pros: live edits bidirectional, zero reinstall on change, preserves git history in GdTimeMachine, fastest inner loop.
- Cons: symlink requires Developer Mode on Windows; breaks if sibling dir is moved/renamed; not hermetic for CI (CI must checkout both repos side-by-side).

### Option B — `local` git source (hermetic dev, still colocated)

```jsonc
{
  "addons": {
    "GdTimeMachine": {
      "url": "../GdTimeMachine",
      "source": "local",
      "checkout": "main",
      "subfolder": "addons/GdTimeMachine"
    }
  }
}
```

- Behaviour: clones from filesystem into `.addons/GdTimeMachine`, then copies subfolder. `checkout` pins branch/tag/commit.
- Pros: real copy (no symlink privilege needed), can pin `checkout: "v0.1.0"` for reproducibility, CI-friendly if filesystem path exists.
- Cons: need `godotenv addons install` after every GdTimeMachine change to refresh; slightly slower.

### Option C — `remote` git URL (Recommended for CI / consumers, not for colocated dev)

```jsonc
{
  "addons": {
    "GdTimeMachine": {
      "url": "https://github.com/WahahaYes/GdTimeMachine",
      "checkout": "v0.1.0",
      "subfolder": "addons/GdTimeMachine"
    }
  }
}
```

- Behaviour: clones from GitHub into `.addons`, copies subfolder.
- Pros: reproducible, works everywhere, no sibling assumption, ready for Asset Library consumers.
- Cons: not colocated — local changes not reflected until pushed/tagged; dev loop is push → reinstall.

### Option D — Git submodule (Not recommended)

Left for completeness. GodotEnv explicitly aims to replace submodules. Submodule fragility on branch switches is the problem GodotEnv solves.

**Recommendation:** Use **A in dev** (`symlink`), with **C for CI/release**. The two can coexist via a small overlay: keep `addons.jsonc` with `gut` + `symlink` for dev, and document the `remote` snippet for CI/forks. Alternatively branch `addons.jsonc` per environment (not needed initially).

## 6. Implementation Checklist (Option A)

1. **Pre-check**

   - `ls ../GdTimeMachine/addons/GdTimeMachine/plugin.cfg` exists; `git -C ../GdTimeMachine status --porcelain` clean.
   - `godotenv --version` and `git --version` ok. `~/.config/gdplanningai-obs/capture_token` preserved if needed for manual fallback.

1. **Update `addons.jsonc`**

   - Add `GdTimeMachine` entry as Option A above. Keep `$schema` and `gut`. Commit message: `chore: vendor GdTimeMachine via godotenv symlink`.

1. **Update `.gitignore`**

   - Ensure:
     ```
     # godotenv — managed addons are not committed
     addons/GdTimeMachine/
     .addons/
     ```
     Current file already has `addons/gut/` + `.addons/`; change to `addons/*` + `!addons/.editorconfig` if you run `godotenv addons init`, or just add `addons/GdTimeMachine/` explicitly to keep the current narrow ignores.

1. **Install**

   ```sh
   godotenv addons install
   ls -l addons/GdTimeMachine   # should be symlink -> ../../GdTimeMachine/addons/GdTimeMachine  (or absolute)
   cat addons/GdTimeMachine/plugin.cfg
   ```

1. **Godot editor verification**

   - Open project, `Project > Project Settings > Plugins → GdTimeMachine: Enabled`.
   - Dock appears bottom panel; backend dropdown shows OBS / Movie Maker / Screenshot.
   - `Editor Settings > gd_time_machine/*` holds obs/recorder/ffmpeg defaults (migrate values from `.env` manually: host/port/password/scene/output_dir).
   - Test record: pick `Screenshot` or `Movie Maker`, scene `examples/hunger_basic_2d.tscn`, duration 5s → `media/captures/<scene>_<ts>.*` appears.

1. **CLI verification (history path)**

   ```sh
   addons/GdTimeMachine/cli/gdtime --help
   godot --headless -s addons/GdTimeMachine/cli/main.gd -- --help
   gdtime doctor --verbose
   gdtime validate test/cli/manifest_history.json   # or your manifest
   gdtime run --dry-run test/cli/manifest_history.json
   ```

   Expect `godotenv godot env target` resolution and worktree creation.

1. **Makefile migration**

   - Keep `launch-editor`, `addons-install`, `godot-pin` etc. (already godotenv-aware).
   - Deprecate `record-obs` / `record-obs-fullscreen`: either delete or turn into shim:
     ```make
     record-obs: ## Record via GdTimeMachine (was OBS python)
       @echo "Use GdTimeMachine dock or: godot --headless -s addons/GdTimeMachine/cli/main.gd -- run <manifest>"
       @false
     ```
     Or delegate to Movie Maker backend if headless still desired — do not keep `uv run scripts/capture_obs.py`.

1. **Manifest migration**

   - Convert `capture_all_showcase.sh`'s hard-coded `MANIFEST=(` array (18 entries) into `GdTimeMachine/cli/schema/batch_manifest.schema.json`-compliant JSON. The shell script's `resolve_godot_bin` logic is now in `core/godot_resolve.gd`.

1. **Commit & push**

   ```sh
   git add addons.jsonc .gitignore notes/GODOTENV_GDTIMEMACHINE_AUDIT.md
   git commit -m "chore: point GdPlanningAI at colocated GdTimeMachine via godotenv"
   ```

## 7. Cleanup / Deprecation (When to Delete Legacy)

Trigger: **after** §6 verification passes (dock + CLI both green). Follow `notes/RECORDING_INVENTORY.md` cleanup checklist, in order:

1. Archive any `media/captures/**/*.mp4` worth keeping (devlog selects).
1. Delete scripts: `scripts/capture_obs.py`, `scripts/obs_controller.py`, `scripts/capture_all_showcase.sh`, `scripts/.obs_capture_token.json` (prune).
1. Delete `.env` + `.env.example` (secrets already migrated to EditorSettings) and `record-obs` Makefile targets/variables (`SCENE/DURATION/FPS/OUTPUT`).
1. `git worktree prune` + `rm -rf .worktrees/` (shared `cargo-target` too).
1. `rm -rf media/captures/` (gitignored; safe after archive).
1. Retire `.gitignore` entries only if unused (`.env` may stay if other tooling uses it; `.worktrees/` + `media/captures/` can stay harmlessly).
1. Keep `notes/obs-recorder-addon/` + `notes/YOUTUBE_SHOWCASE_COMMITS.md` unless folding into `../GdTimeMachine` (they are the design provenance).
1. Update `notes/RECORDING_INVENTORY.md` header to `Status: Superseded by GdTimeMachine via godotenv (2026-09-03)` with link here.

Do **not** delete end-to-end example scenes (`examples/*.tscn`) — they are demo content, not recording infra.

## 8. Risks & Open Questions

- **Path portability:** `symlink` assumes sibling layout `/home/ethan/repos/GdPlanningAI` ↔ `../GdTimeMachine`. Contributors cloning to different paths will break unless they replicate layout or switch to `remote` URL. Mitigation: document layout + provide `remote` snippet in README; CI checks out both repos adjacent.
- **Windows symlink:** Requires Developer Mode or admin. Fallback to `local` for Windows contributors.
- **`.addons` cache:** Must stay gitignored and not committed; `godotenv addons install` is required after fresh clone. Add to onboarding docs.
- **Version pinning:** `symlink` has no `checkout` — dev tracks `main` tip. For release, pin `remote` to `v0.1.0` tag (GdTimeMachine's tagged release) to avoid float.
- **OBS credentials:** Moving from `.env` (ignored) to `EditorSettings` stores password in `~/.config/Godot/editor_settings-4.tres` (plaintext, per-machine). Acceptable but note in docs; do not commit.
- **Asset Library:** Once GdTimeMachine is on Asset Library, consumers won't need godotenv at all; colocated godotenv is a **dev-only** mechanism. Keep both paths documented.
- **Godot version resolution:** `core/godot_resolve.gd` + `godotenv godot use` replaces shell `resolve_godot_bin`. Ensure `.godotrc` / project.godot `config/features` mapping still works on old commits (tested via `gdtime doctor`).
- **Existing automation:** Any CI that calls `make record-obs` will break after deletion — migrate to `gdtime run <manifest>` before removing.

______________________________________________________________________

*Next step:* Reviewer approves Option A; implement §6 and validate dock + `gdtime doctor --verbose` + one dry-run batch before proceeding to §7 deletions.
