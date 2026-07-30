# Implementation Plan: GdTimeMachine

Date: 2026-07-29
Based on: `ARCHITECTURE.md`, Metis pre-planning analysis

---

## Corrections to the original architecture

Before the plan, key changes Metis identified:

| Issue | Architecture sketch | Corrected |
|-------|-------------------|-----------|
| Backend base class | `extends RefCounted` | **`extends Node`** — ObsWebSocket is a Node and needs scene tree |
| Movie Maker `is_available()` | Returns `EditorInterface.is_movie_maker_enabled()` | **Always `true`** — Movie Maker is built-in, always available |
| Output format | Unspecified | **`.avi`** — Movie Maker's native output. Not `.mp4` |
| CLI launch | `OS.execute(...)` | **`OS.create_process()`** — `execute()` blocks the editor |
| ObsWebSocket vendoring | `vendor/obs_websocket.gd` (single file) | **`vendor/obs_websocket_gd/`** (directory — the library is multi-file) |
| Settings | `settings.gd` Resource + EditorSettings | **EditorSettings only** — drop the Resource. No `settings_ui.tscn` |
| BackendMovieMakerCLI | In v1 | **Deferred to v2** — low value, high complexity |
| Signal routing | Dock → Backend directly | **RecorderController** — a controller node that re-emits from the active backend |

---

## Build phases

### Phase 0 — Scaffold (1–2 hrs)

Create the addon skeleton. No backends yet, just a plugin that Godot recognizes.

**Files to create:**
```
addons/gd-time-machine/
├── plugin.cfg
├── plugin.gd
└── README.md
```

**`plugin.cfg`:**
```ini
[plugin]
name="GdTimeMachine"
description="Time machine for your Godot project. Rewind any commit, record any scene — via OBS or built-in Movie Maker."
author="Ethan Wilson"
version="0.1.0"
script="plugin.gd"
```

**`plugin.gd`** (skeleton):
```gdscript
@tool
extends EditorPlugin

var _recorder_controller: Node

func _enter_tree() -> void:
    _recorder_controller = preload("...controller.gd").new()
    add_child(_recorder_controller)

func _exit_tree() -> void:
    if _recorder_controller:
        _recorder_controller.stop_recording_if_active()
        remove_child(_recorder_controller)
        _recorder_controller.queue_free()
```

**Acceptance:**
- Copy `addons/gd-time-machine/` into a test project
- Enable plugin in Project Settings → Plugins
- Verify no errors in the editor output

---

### Phase 1 — RecorderController + RecorderBackend base (2–3 hrs)

The core abstraction layer. Everything else builds on this.

**Files to create:**
```
addons/gd-time-machine/
├── controller/
│   └── recorder_controller.gd     # Signal router, backend lifecycle
└── backend/
    └── recorder_backend.gd         # Abstract base class (Node)
```

**`backend/recorder_backend.gd`** — abstract base:
```gdscript
@tool
extends Node
class_name RecorderBackend

func get_name() -> String:
    return ""           # "OBS Studio", "Godot Movie Maker"

func get_description() -> String:
    return ""

func is_available() -> bool:
    return false        # prerequisites met?

func is_recording() -> bool:
    return false

func start(config: Dictionary) -> void:
    # Keys: output_path, fps, duration, scene_path, fullscreen
    pass

func stop() -> void:
    pass

signal recording_started(backend_name, output_path)
signal recording_stopped(backend_name, output_path)
signal recording_progress(backend_name, elapsed_sec)
signal recording_error(backend_name, error_message)
```

**`controller/recorder_controller.gd`** — owns the active backend, re-emits signals:
```gdscript
@tool
extends Node

var backends: Dictionary = {}       # name → RecorderBackend instance
var active_backend: RecorderBackend = null

func register_backend(backend: RecorderBackend) -> void: ...
func select_backend(name: String) -> void: ...
func start_recording(config: Dictionary) -> void: ...
func stop_recording() -> bool: ...

signal backend_changed(backend_name)
signal recording_started(backend_name, output_path)
signal recording_stopped(backend_name, output_path)
signal recording_progress(backend_name, elapsed_sec)
signal recording_error(backend_name, error_message)
```

**Acceptance:**
- GUT test instantiates `RecorderController`, registers a mock backend
- Signals route correctly through the controller
- `select_backend("nonexistent")` fails gracefully

---

### Phase 2 — BackendMovieMaker + Dock UI (4–6 hrs)

The "works immediately" path. After this phase, the addon can record scenes with zero dependencies.

**Files to create:**
```
addons/gd-time-machine/
├── backend/
│   └── backend_movie_maker.gd
├── ui/
│   ├── recorder_dock.gd
│   ├── recorder_dock.tscn
│   └── icons/
│       ├── icon_record.svg     or .png
│       └── icon_stop.svg
```

**`backend/backend_movie_maker.gd`:**
```gdscript
@tool
extends RecorderBackend
class_name BackendMovieMaker

func get_name() -> String:    return "Godot Movie Maker"
func get_description() -> String:  return "Built-in Godot encoder. No extra software needed."

func is_available() -> bool:  return true   # always available in Godot 4

func is_recording() -> bool:  return EditorInterface.is_movie_maker_enabled()

func start(config: Dictionary) -> void:
    ProjectSettings.set_setting("editor/movie_writer/movie_file", config.output_path)
    ProjectSettings.save()
    EditorInterface.set_movie_maker_enabled(true)
    # Launch the scene
    if config.has("scene_path"):
        EditorInterface.play_custom_scene(config.scene_path)

func stop() -> void:
    EditorInterface.set_movie_maker_enabled(false)
    EditorInterface.stop_playing()
```

**Start detection:** Connect to `EditorInterface.play_mode_changed` signal or poll `EditorInterface.is_playing_scene()` via a Timer.

**Stop detection:** Use a Timer (e.g., 0.5s interval) that polls `EditorInterface.is_playing_scene()`. When playback stops, emit `recording_stopped`. This catches both manual stop (user clicks Stop) and natural exit (scene closes).

**`ui/recorder_dock.tscn` layout:**
```
VBoxContainer
├── HBoxContainer — Title
│   ├── TextureRect — icon
│   ├── Label — "Godot Recorder"
│   └── Button — ⚙ settings
├── HSeparator
├── HBoxContainer — Backend selector
│   ├── Label — "Backend:"
│   └── OptionButton — populated from controller.backends
├── VBoxContainer — Settings group
│   ├── HBoxContainer — Scene
│   │   ├── LineEdit (editable, populated from current scene)
│   │   └── Button — "Use Current"
│   ├── HBoxContainer — Duration
│   │   ├── SpinBox (1–300, default 30)
│   │   └── Label — "seconds"
│   └── HBoxContainer — FPS
│       ├── SpinBox (15–240, default 60)
│       └── Label — "fps"
├── HBoxContainer — Output
│   ├── Label — "→"
│   └── LineEdit (default: "res://media/captures/")
├── HBoxContainer — Status
│   ├── Label — "Status: Ready"
│   └── TextureRect — status indicator (green/red/grey)
├── Button — "● Record" / "■ Stop"  (toggles)
└── VBoxContainer — Recent captures
    ├── Label — "Recent captures:"
    └── ItemList — populated by scanning output dir
```

**`ui/recorder_dock.gd`:**
```gdscript
@tool
extends VBoxContainer

var controller: RecorderController

func _ready() -> void:
    controller = find_parent("EditorPlugin").get_node("RecorderController")
    controller.recording_started.connect(_on_recording_started)
    controller.recording_stopped.connect(_on_recording_stopped)
    # populate backend selector from controller.backends

func _on_record_pressed() -> void:
    if controller.is_recording():
        controller.stop_recording()
    else:
        controller.start_recording({
            output_path = _build_output_path(),
            scene_path = $ScenePath.text,
            fps = $FPS.value,
        })
```

**Acceptance:**
- Plugin loads, dock appears, Movie Maker backend is selectable
- Click "Record" → scene starts playing → `.avi` file appears in output directory
- Click "Stop" or let scene exit → recording stops, file is playable
- GUT test: `BackendMovieMaker.new().is_available() == true`

---

### Phase 3 — Vendor obs-websocket-gd (1–2 hrs)

Fetch the OBS WebSocket GDScript library and verify it can handshake with a running OBS instance.

**Files to create:**
```
addons/gd-time-machine/
└── vendor/
    └── obs_websocket_gd/
        ├── obs_websocket.gd        # Main file from upstream
        ├── obs_websocket.tscn      # Scene wrapper (optional)
        ├── package.json            # Upstream metadata, for attribution
        └── LICENSE                 # Apache-2.0 (upstream license)
```

**Steps:**
1. Clone `https://github.com/you-win/obs-websocket-gd`
2. Copy `addons/obs-websocket-gd/obs_websocket.gd` into `vendor/obs_websocket_gd/`
3. Also copy `addons/obs-websocket-gd/obs_websocket.tscn` if it exists
4. Copy `LICENSE` for attribution
5. Add a `NOTICE.txt` in the vendor directory: _"Includes obs-websocket-gd (Apache-2.0) by you-win"_
6. Do **not** include `plugin.cfg` or `plugin.gd` from upstream — vendored as a library, not a plugin

**Verify in isolation:**
```gdscript
var obs = preload("vendor/obs_websocket_gd/obs_websocket.gd").new()
add_child(obs)
obs.host = "localhost"
obs.port = 4455
obs.password = EditorSettings.get_setting("godot_obs_recorder/obs/password")
obs.establish_connection()
# → expect connection_authenticated signal within 5 seconds
```

**Acceptance:**
- OBS running with WebSocket enabled on port 4455
- Fresh `obs_websocket` node connects and authenticates
- Connection failure (wrong password, OBS not running) reports error gracefully — no crash

---

### Phase 4 — BackendOBS (4–6 hrs)

Port the existing Python `obs_controller.py` logic into GDScript. This is the premium path.

**Files to create:**
```
addons/gd-time-machine/
└── backend/
    ├── backend_obs.gd              # Main OBS backend
    └── platform_capture.gd         # Platform-specific capture source helpers
```

**`backend/backend_obs.gd`** — ports `obs_controller.py` to GDScript:

```gdscript
@tool
extends RecorderBackend
class_name BackendOBS

var _obs: Node          # instance of obs_websocket.gd
var _godot_proc_pid: int = -1  # for CLI-launched instances

func get_name() -> String:    return "OBS Studio"
func get_description() -> String:  return "Real-time capture via OBS. No framerate bottleneck."

func is_available() -> bool:
    # Try connecting to OBS WebSocket — if it responds, OBS is running
    return _probe_obs()

func start(config: Dictionary) -> void:
    # 1. Connect to OBS WebSocket
    # 2. Ensure capture source exists (platform-specific)
    #    - Linux/PipeWire: create "pipewire-screen-capture-source" with RestoreToken
    #    - Windows: create "window_capture" matching godot binary
    #    - macOS: create "display_capture" (or window_capture)
    # 3. Optionally switch OBS scene
    # 4. Start recording via OBS WebSocket
    # 5. Launch Godot scene via OS.create_process() if scene_path given
    # 6. Poll recording state and scene process
    # 7. When done → StopRecord, move file

func stop() -> void:
    # Stop OBS recording
    # Kill Godot subprocess if we launched one
    # Move output file to configured directory
```

**`backend/platform_capture.gd`** — static helpers for OBS capture source creation:

```gdscript
static func get_preferred_capture_kind() -> String:
    match OS.get_name():
        "Linux", "FreeBSD":
            # Try PipeWire → Xcomposite fallback
            if _has_pipewire():
                return "pipewire-screen-capture-source"
            return "xcomposite_screen"
        "Windows":
            return "window_capture"  # or "game_capture"
        "macOS":
            return "display_capture"
        _:
            return ""

static func create_capture_settings(platform_kind: String, token: String = "") -> Dictionary:
    match platform_kind:
        "pipewire-screen-capture-source":
            return { "RestoreToken": token } if token else {}
        "window_capture":
            return { "window": "godot*", "capture_method": "bitblt" }
        "display_capture":
            return {}
```

**Token persistence** — ported from Python's `load_monitor_token()` / `persist_monitor_token()`:
- Store in EditorSettings under `"godot_obs_recorder/obs/pipewire_token"`
- First-run flow: user creates source in OBS → addon reads back `RestoreToken` → persists

**Guided Wayland setup (`needs_setup() -> bool`):**
```gdscript
# BackendOBS adds a method the dock checks:
func needs_setup() -> bool:
    if OS.get_name() != "Linux" and OS.get_name() != "FreeBSD":
        return false  # other platforms don't need token setup
    if EditorSettings.get_setting("godot_obs_recorder/obs/pipewire_token"):
        return false  # already have a token
    # Check if OBS already has a capture source with a token
    return not _has_active_capture_token()
```

When `needs_setup()` is true, the dock shows:
```
┌──────────────────────────────────────┐
│  ⚠ OBS Capture Source Not Configured │
│                                       │
│  OBS needs a screen capture source    │
│  to record. Let's set one up.         │
│                                       │
│  [Step 1] Open OBS Scene Editor       │
│  [Step 2] Add "Screen Capture" source │
│          (you'll pick your monitor)   │
│  [Step 3] Click "Detect & Save" →     │
│                                       │
│  Status: ◌ Waiting for source...      │
│                                       │
│  [Detect Capture Source]  [Skip]      │
└──────────────────────────────────────┘
```

The "Detect Capture Source" button calls `_read_active_token()` (ported from Python) via OBS WebSocket. If a capture source with a valid RestoreToken exists in the active scene, the addon reads it back and persists it. No portal interaction needed — the portal dialog already happened when the user added the source in OBS.

**Acceptance:**
- OBS running, backend shows "available" in dock
- Connect → OBS starts recording → Godot scene plays
- Stop → recording file saved to output directory
- Linux/Wayland: RestoreToken persists across OBS restarts
- Linux/Wayland first-run: setup flow works (user selects monitor in portal, addon captures token)
- Graceful error when OBS not running

---

### Phase 5 — Polish + FFmpeg Transcode + Settings Integration (3–4 hrs)

Wire everything together with proper settings and error handling. Add optional ffmpeg transcode step.

**Settings schema (EditorSettings, prefix `godot_obs_recorder/`):**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `obs/host` | String | `"localhost"` | OBS WebSocket host |
| `obs/port` | int | `4455` | OBS WebSocket port |
| `obs/password` | String | `""` | OBS WebSocket password |
| `obs/scene` | String | `"Scene"` | OBS scene to use |
| `obs/auto_launch` | bool | `false` | Auto-start OBS if not running |
| `obs/pipewire_token` | String | `""` | Persisted PipeWire RestoreToken |
| `recorder/default_backend` | String | `"Godot Movie Maker"` | Default backend |
| `recorder/default_duration` | int | `30` | Default recording duration |
| `recorder/default_fps` | int | `60` | Default FPS cap |
| `recorder/output_dir` | String | `"res://media/captures"` | Default output directory |
| `recorder/fullscreen` | bool | `true` | Launch in fullscreen |

**Plugin.gd registers settings on `_enter_tree()`:**
```gdscript
func _register_settings() -> void:
    var es = EditorInterface.get_editor_settings()
    var settings = {
        "godot_obs_recorder/obs/host": { "type": TYPE_STRING, "default": "localhost" },
        "godot_obs_recorder/obs/port": { "type": TYPE_INT, "default": 4455 },
        "godot_obs_recorder/obs/password": { "type": TYPE_STRING, "default": "" },
        # ... etc
    }
    for key in settings:
        if not es.has_setting(key):
            es.set_setting(key, settings[key]["default"])
            es.add_property_info({
                "name": key,
                "type": settings[key]["type"],
            })
```

**Optional ffmpeg transcode:**
- After recording stops, if the setting `recorder/transcode_to_mp4` is enabled:
  ```gdscript
  func _transcode_to_mp4(input_path: String) -> void:
      var output_path = input_path.trim_suffix(".avi") + ".mp4"
      var err = OS.execute("ffmpeg", [
          "-i", input_path,
          "-c:v", "libx264",
          "-preset", "fast",
          "-crf", "18",
          "-y",
          output_path
      ])
      if err == OK:
          # Optionally delete the original .avi (configurable)
          DirAccess.remove_absolute(input_path)
          return output_path
  ```
- Setting: `recorder/transcode_to_mp4` (bool, default false)
- Setting: `recorder/transcode_delete_source` (bool, default true)
- Document that ffmpeg must be installed and on PATH
- Transcode runs in a background thread or via a non-blocking `OS.create_process()`, dock shows "Converting..." status

**Cleanup on `_exit_tree()`:**
```gdscript
func _exit_tree() -> void:
    if _controller and _controller.is_recording():
        _controller.stop_recording()
    if _controller:
        remove_child(_controller)
        _controller.queue_free()
    _unregister_settings()

func _unregister_settings() -> void:
    # EditorSettings doesn't have a remove_setting() — settings persist.
    # That's fine — they just won't be used if the addon is disabled.
    pass
```

---

## File manifest (complete)

```
addons/gd-time-machine/
├── plugin.cfg                          # Phase 0
├── plugin.gd                           # Phase 0, updated Phase 5
├── README.md                           # Phase 0
├── LICENSE                             # Phase 0 (Apache-2.0)
├── NOTICE.txt                          # Phase 3 (vendor attribution)
│
├── controller/
│   └── recorder_controller.gd          # Phase 1
│
├── backend/
│   ├── recorder_backend.gd             # Phase 1
│   ├── backend_movie_maker.gd          # Phase 2
│   ├── backend_obs.gd                  # Phase 4
│   └── platform_capture.gd             # Phase 4
│
├── ui/
│   ├── recorder_dock.gd                # Phase 2
│   ├── recorder_dock.tscn              # Phase 2
│   └── icons/
│       ├── icon_recorder.svg
│       ├── icon_record.svg
│       └── icon_stop.svg
│
├── vendor/
│   └── obs_websocket_gd/
│       ├── obs_websocket.gd            # Phase 3
│       ├── LICENSE                     # Phase 3
│       └── NOTICE.txt                  # Phase 3
│
└── test/
    ├── unit/
    │   ├── test_recorder_backend.gd
    │   ├── test_backend_movie_maker.gd
    │   └── test_backend_obs.gd
    └── integration/
        └── test_obs_connection.gd
```

---

## Testing strategy

| What to test | How | Runs in GUT headless? |
|-------------|-----|----------------------|
| `RecorderBackend` abstract interface | Instantiate, call methods, check contracts | ✅ Yes |
| `BackendMovieMaker` logic | Instantiate, test `get_name()`, `is_available()`, state transitions | ✅ Yes (most) |
| `BackendMovieMaker.start()` output | Manual — run a scene, check `.avi` appears. Automated with headless godot + `--write-movie` | ⚠️ Requires headless test |
| `BackendOBS` logic | Instantiate, test platform detection, token persistence | ✅ Yes |
| `BackendOBS` connection | Manual — requires running OBS with WebSocket | ❌ No (skip in CI) |
| `obs-websocket-gd` handshake | Manual — requires running OBS | ❌ No |
| Plugin lifecycle (dock appears, plugin reload) | Manual — open editor, enable/disable | ❌ No |
| GUT suite | `make test-godot` | ✅ Yes |

**Test files per GUT convention (existing pattern: `test/unit/test_*.gd`):**
- `test/unit/test_recorder_backend.gd` — instantiate `RecorderBackend`, verify it extends Node
- `test/unit/test_backend_movie_maker.gd` — mock `EditorInterface`, test state machine
- `test/unit/test_backend_obs.gd` — test platform detection, token persistence
- `test/unit/test_platform_capture.gd` — test `get_preferred_capture_kind()` returns per OS

---

## Estimated total effort

| Phase | What | Time | Parallelizable? |
|-------|------|------|----------------|
| 0 | Plugin skeleton | 1–2 hrs | — |
| 1 | Controller + Backend base | 2–3 hrs | — |
| 2 | Movie Maker + Dock UI | 4–6 hrs | Phase 2 & 3 are parallel |
| 3 | Vendor obs-websocket-gd | 1–2 hrs | ✅ With Phase 2 |
| 4 | OBS Backend | 4–6 hrs | After Phase 3 |
| 5 | Polish + Settings | 2–3 hrs | After 2, 4 |
| **Total** | | **14–22 hrs** | |

**Time to first value (Phase 2 complete):** ~7–11 hours. At that point you have a working addon with the Movie Maker path.

---

## GdTimeMachine Historical Capture — design from day one

The historical commit capture (CLI companion) is GdTimeMachine's **core differentiator** — no other Godot recording tool can rewind your project to any commit, resolve the right Godot version via godotenv, rebuild native extensions, and record. This is what makes GdTimeMachine a time machine for your project's visual history.

The manifest format and batch concepts are designed into v0.1 even though the CLI execution comes later:

- The **RecorderBackend base class** already accepts a `Dictionary` config — a batch manifest entry maps 1:1 to that config shape.
- GdTimeMachine's dock UI should **reserve space** for the batch manifest editor (even if hidden in v0.1), so the manifest JSON format is stable before the CLI tool exists.
- The **batch JSON schema** is defined during v0.1 development, not retrofitted.

This means Phase 5 includes defining the manifest JSON schema and adding a "Export Manifest" button that serializes a single capture config to the format the CLI will consume later.

## Naming reference

| Thing | Name |
|-------|------|
| Addon directory | `addons/gd-time-machine/` |
| Plugin name (in `plugin.cfg`) | `GdTimeMachine` |
| CLI companion (future) | `gdtime-cli` (or `gdtm-cli`) |
| Scene dock | `res://addons/gd-time-machine/ui/time_machine_dock.tscn` |
| Settings prefix | `gd_time_machine/` |

## Future enhancements (not in v0.1 scope)

These are captured for later.

| Feature | Source | Notes |
|---------|--------|-------|
| **"Record That" replay buffer** | `BRAINSTORM.md` | OBS replay buffer saves last N seconds on demand. Like NVIDIA Shadowplay for Godot dev — zero overhead until you hit save. Triggerable via dock button or hotkey. Requires OBS replay buffer to be pre-configured. |
| **CLI companion execution** | `ENHANCEMENT_CLI_COMPANION.md` | The CLI tool itself — reads the manifest JSON, runs the git worktree + godotenv + Rust rebuild + record loop. Python in v1, potentially Rust later. |
| **Batch recording UI** | `ARCHITECTURE.md`, `ENHANCEMENT_CLI_COMPANION.md` | Full GUI manifest editor in GdTimeMachine dock (add/remove/reorder entries, pick commits from git log, drag-drop scenes). Exports JSON that the CLI companion consumes. |
| **CI integration** | `BRAINSTORM.md` | Automated capture as CI artifacts for visual diff / PR review. The manifest JSON format makes this straightforward — the CLI companion can run in CI without any UI. |
| **Windows/macOS OBS backends** | `RESEARCH.md` | Platform-specific capture source creation for Windows (window_capture) and macOS (display_capture). Documented in `platform_capture.gd` as stub branches. |
| **`BackendMovieMakerCLI`** | `ARCHITECTURE.md` | Launch separate Godot instance via `OS.create_process()`. Non-blocking, but complex process lifecycle management. Marginal value over the editor-integrated Movie Maker backend. |

## Decisions (confirmed)

| Question | Decision | Implication |
|----------|----------|-------------|
| Movie Maker output format | **Accept `.avi`, add optional ffmpeg transcode** | Phase 5 gets a "Convert to MP4" toggle. When enabled, `OS.execute("ffmpeg", ...)` runs after recording stops. Document that ffmpeg must be on PATH. |
| Wayland first-run UX | **Guided setup** | BackendOBS gets a `needs_setup() -> bool` method. When `true`, the dock shows a "Setup Capture Source" button that steps the user through the portal dialog, reads back the token, and persists it. |
| OBS backend scope | **Linux-only for v1**, Win/Mac documented as future | `platform_capture.gd` only implements PipeWire/X11. Windows/macOS branches get `return ""` with a `push_warning()`. Documented in README as "Coming soon — PRs welcome." |
| `BackendMovieMakerCLI` | **Deferred** | No file created. If needed later, it gets its own phase. |
| Repository | **Separate repo on launch** | Develop under this project's `addons/` for now. When ready for Asset Library, spin out to `github.com/WahahaYes/godot-obs-recorder`. Separate git history, independent versioning. |
