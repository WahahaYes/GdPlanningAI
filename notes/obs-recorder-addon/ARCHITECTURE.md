# GdTimeMachine — Architecture Sketch

Date: 2026-07-29 Status: Pre-implementation design

______________________________________________________________________

## Addon directory layout

```
addons/gd-time-machine/
├── plugin.cfg                      # Addon metadata (name, version, author)
├── plugin.gd                       # EditorPlugin — lifecycle, dock, toolbar
│
├── backend/
│   ├── recorder_backend.gd         # Abstract base class (RefCounted)
│   ├── backend_obs.gd              # OBS backend — wraps obs-websocket-gd
│   ├── backend_movie_maker.gd      # Godot Movie Maker — EditorInterface API
│   └── backend_movie_maker_cli.gd  # Movie Maker via OS.execute() launch
│
├── vendor/
│   └── obs_websocket.gd            # Vendored obs-websocket-gd (Apache-2.0)
│
├── ui/
│   ├── recorder_dock.gd            # Dock panel controller
│   ├── recorder_dock.tscn          # Dock panel scene
│   ├── recorder_dock_theme.tres    # Visual theme
│   └── icons/
│       ├── icon_recorder.svg       # General recorder icon
│       ├── icon_record.svg         # Record button
│       └── icon_stop.svg           # Stop button
│
├── settings.gd                     # Settings resource (exported vars)
├── settings_ui.gd                  # Settings dialog logic
├── settings_ui.tscn                # Settings dialog scene
│
├── LICENSE                         # Apache-2.0
└── README.md                       # Addon docs
```

______________________________________________________________________

## Core flow

```
 User clicks [● Record]
        │
        ▼
  recorder_dock.gd
        │
        ├── backend.start_recording(config)
        │       │
        │       ├── [OBS backend]
        │       │   ├── obs_websocket.establish_connection()
        │       │   ├── obs_websocket.send_command("StartRecord")
        │       │   └── signal: recording_started
        │       │
        │       ├── [Movie Maker (editor)]
        │       │   ├── ProjectSettings.set("editor/movie_writer/movie_file", path)
        │       │   ├── EditorInterface.set_movie_maker_enabled(true)
        │       │   ├── EditorInterface.play_custom_scene(scene)
        │       │   └── → recording stops when editor stops playing
        │       │
        │       └── [Movie Maker (CLI)]
        │           ├── OS.execute("godot", ["--write-movie", clip, "--path", dir])
        │           └── → recording stops when launched instance exits
        │
        └── signals: recording_progress, recording_finished, recording_error
```

______________________________________________________________________

## RecorderBackend (abstract base)

```gdscript
# backend/recorder_backend.gd
class_name RecorderBackend
extends RefCounted

# ── Query ──────────────────────────────────────────
func get_name() -> String:           # Human-readable name
func get_description() -> String:    # Short summary for tooltip
func is_available() -> bool:         # Prerequisites met?
func is_recording() -> bool:         # Currently recording?

# ── Lifecycle ──────────────────────────────────────
func start(config: Dictionary) -> void:
    # Keys: output_path, fps, duration, scene_path, fullscreen
func stop() -> void:

# ── Signals (use a Node for signal emission) ───────
signal recording_started(backend_name, output_path)
signal recording_stopped(backend_name, output_path)
signal recording_error(backend_name, error_message)
```

______________________________________________________________________

## OBS backend detail

```gdscript
# backend/backend_obs.gd
class_name BackendOBS
extends RecorderBackend

# Owns an obs_websocket_gd node (added to editor's scene tree)
var _obs: ObsWebSocket  # from vendor/obs_websocket.gd

# Per-platform capture source setup
var _capture_source_creator: PlatformCapture

func start(config: Dictionary) -> void:
    # 1. Connect to OBS WebSocket (localhost:4455 with password)
    # 2. Ensure capture source exists (platform-specific)
    #    - Linux/PipeWire → create "pipewire-screen-capture-source"
    #    - Windows       → create "window_capture" matching "godot"
    #    - macOS         → create "display_capture"
    # 3. Optionally switch OBS scene
    # 4. Launch Godot scene (if config says to):
    #    OS.execute("godot", ["--scene", path, "--max-fps", fps])
    # 5. Call StopRecord
    # 6. Wait for Godot process to exit, move file
```

______________________________________________________________________

## Movie Maker backend (editor)

```gdscript
# backend/backend_movie_maker.gd
class_name BackendMovieMaker
extends RecorderBackend

func is_available() -> bool:
    return EditorInterface.is_movie_maker_enabled()  # actually checks capability
    # Always true in Godot 4 — Movie Maker is built-in

func start(config: Dictionary) -> void:
    # 1. Set output path in project settings
    ProjectSettings.set_setting("editor/movie_writer/movie_file", config.output_path)
    ProjectSettings.save()  # persist for the run

    # 2. Enable Movie Maker
    EditorInterface.set_movie_maker_enabled(true)

    # 3. Launch the scene
    if config.get("scene_path"):
        EditorInterface.play_custom_scene(config.scene_path)
    else:
        EditorInterface.play_main_scene()

    # 4. Recording is active until the user stops playback
    #    (poll is_recording() or listen for MainSceneChanged)

func stop() -> void:
    EditorInterface.set_movie_maker_enabled(false)
    EditorInterface.stop_playing()
```

______________________________________________________________________

## Plugin entry point

```gdscript
# plugin.gd
@tool
extends EditorPlugin

var _dock: Control
var _backends: Dictionary  # { name: RecorderBackend }
var _active_backend: RecorderBackend

func _enter_tree() -> void:
    # Load dock UI
    _dock = preload("ui/recorder_dock.tscn").instantiate()
    add_control_to_dock(DOCK_SLOT_LEFT_UR, _dock)

    # Register available backends
    _register_backend(BackendOBS.new())
    _register_backend(BackendMovieMaker.new())
    _register_backend(BackendMovieMakerCLI.new())

    # Add toolbar button
    add_tool_menu_item("Record Scene...", _on_toolbar_record)

func _exit_tree() -> void:
    remove_control_from_dock(_dock)
    _dock.free()
    remove_tool_menu_item("Record Scene...")
```

______________________________________________________________________

## Dock panel layout

```
┌──────────────────────────────────┐
│  ● GdTimeMachine           [⚙]  │  ← title bar + settings
├──────────────────────────────────┤
│  Backend:  [OBS Studio       ▼]  │  ← backend selector
│  Status:  ● Connected (OBS v31)  │  ← live status
├──────────────────────────────────┤
│  Scene:   [Current Scene     ▼]  │  ← scene picker
│  Duration: [⏱ 30] seconds       │
│  FPS cap:  [60 ▼]               │
│  Output:   /media/captures/      │
│           [●] [Browse...]        │
├──────────────────────────────────┤
│                                   │
│      [ ● Start Recording ]       │  ← big button, changes to
│                                   │    [ ■ Stop ] when active
├──────────────────────────────────┤
│  Recent captures:                 │
│  ├ 2026-07-28_campfire.mp4  [▶]  │
│  └ 2026-07-28_stress.mp4    [▶]  │
└──────────────────────────────────┘
```

______________________________________________________________________

## Settings panel

Wraps these persisted settings (stored in addon's own config file, not project.godot):

| Setting | Key | Default | |---------|-----|---------| | OBS host | `gd_time_machine/obs/host` | `localhost` | | OBS port | `gd_time_machine/obs/port` | `4455` | | OBS password | `gd_time_machine/obs/password` | (empty, stored in editor config) | | OBS scene name | `gd_time_machine/obs/scene` | `Scene` | | Default backend | `gd_time_machine/recorder/default_backend` | `obs` | | Default duration | `gd_time_machine/recorder/default_duration` | `30` | | Default FPS | `gd_time_machine/recorder/default_fps` | `60` | | Output directory | `gd_time_machine/recorder/output_dir` | `res://media/captures` | | Launch OBS automatically | `gd_time_machine/obs/auto_launch` | `false` | | Fullscreen mode | `gd_time_machine/recorder/fullscreen` | `true` |

Stored in `EditorInterface.get_editor_settings()` under `"gd_time_machine/"` prefix — doesn't pollute project settings.

______________________________________________________________________

## Backend selection UX

The dock's backend dropdown shows each backend with its availability:

```
[OBS Studio             ]   ● available
[Godot Movie Maker      ]   ● available
[Godot Movie Maker (CLI)]   ⚠ limited (no audio config)
```

Selection is remembered per-project. Fallback: if OBS backend is selected but OBS isn't running, show a warning toast but still allow using it (they can start OBS).

______________________________________________________________________

## Open design questions to resolve later

1. **OBS password storage**: Editor settings are plaintext. Should we warn users?
1. **Audit output size**: Movie Maker produces .avi — user wants mp4? Transcode after?
1. **Progress indication**: OBS recording has no progress — we just wait. Show countdown timer?
1. **Multi-scene batch**: Like our `capture_all_showcase.sh` but with a GUI manifest editor. Phase 2.
1. **Replay buffer**: The "Record That" button (saves last N seconds) — cool but requires OBS replay buffer to be active.

______________________________________________________________________

## Dependency graph

```
plugin.gd
  ├── ui/recorder_dock.gd
  │     └── backend/recorder_backend.gd
  │           ├── backend/backend_obs.gd
  │           │     ├── vendor/obs_websocket.gd
  │           │     └── (platform capture source creation — inline)
  │           ├── backend/backend_movie_maker.gd
  │           └── backend/backend_movie_maker_cli.gd
  ├── settings.gd
  └── settings_ui.gd
```

No circular deps. Backends are loaded on-demand, not all at startup.

______________________________________________________________________

## First implementation target

For the first cut, I'd implement:

1. `plugin.gd` + `plugin.cfg` — skeleton plugin that registers a dock
1. `backend/recorder_backend.gd` — abstract base
1. `backend/backend_movie_maker.gd` — the zero-dep path, works immediately
1. `ui/recorder_dock.gd` + `ui/recorder_dock.tscn` — the UI shell
1. `backend/backend_obs.gd` — Linux-only OBS backend (our existing logic ported to GDScript)
1. `vendor/obs_websocket.gd` — vendored, untouched

This gives a working addon with the Movie Maker path day one, and OBS as a bonus for users who have it.
