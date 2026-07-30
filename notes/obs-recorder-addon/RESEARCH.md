# OBS Recorder Godot Addon — Research & Brainstorming

Date: 2026-07-29

## Table of Contents

1. [What We Built](#1-what-we-built)
2. [Cross-Platform Feasibility](#2-cross-platform-feasibility)
3. [Existing Tooling Landscape](#3-existing-tooling-landscape)
4. [Editor Integration Approaches](#4-editor-integration-approaches)
5. [Architecture Brainstorming](#5-architecture-brainstorming)
6. [Open Questions & Next Steps](#6-open-questions--next-steps)

---

## 1. What We Built

### Current pipeline (scripts/)

The existing OBS recording pipeline lives in `scripts/` and consists of three files:

| File | Purpose |
|------|---------|
| `capture_obs.py` | CLI entrypoint: launches Godot scene, connects OBS, records, cleans up |
| `obs_controller.py` | OBS WebSocket controller: creates PipeWire capture source, manages recording |
| `capture_all_showcase.sh` | Orchestrator: iterates over manifest of commits, uses git worktrees, builds Rust, runs per-commit captures |

### How it works

```
capture_all_showcase.sh
  ├── git worktree add <commit>        # Check out historical commit
  ├── resolve_godot_bin                # Find correct Godot version via godotenv
  ├── cargo build (debug)              # Build Rust GDExtension
  ├── godot --editor --quit            # Regenerate .godot/ resource cache
  └── capture_obs.py <scene> -d N -f --start-obs
        ├── launch Godot --scene <path> --max-fps 60
        ├── connect to OBS WebSocket (obsws-python)
        ├── ensure_screen_capture()    # Create PipeWire source with RestoreToken
        ├── start_recording()
        ├── sleep(duration) or wait for Godot exit
        ├── stop_recording()
        ├── shutil.move() output       # Move to media/captures/
        └── kill OBS (SIGKILL to skip "open streams" modal)
```

### Key details

- **OBS WebSocket v5** — built into OBS Studio 28+. Uses `obsws-python` library.
- **PipeWire screen capture** — Linux Wayland only, uses `RestoreToken` from `xdg-desktop-portal` to persist monitor selection across sessions.
- **Token persistence** — saves `RestoreToken` to both `~/.config/gdplanningai-obs/capture_token` and `scripts/.obs_capture_token.json`.
- **Godot launch** — via `subprocess.Popen` with `--scene`, `--max-fps`, optional `-f` fullscreen.
- **No built-in Godot integration** — purely CLI Python scripts.

### What it's used for

The primary use case is automating the capture of showcase footage across multiple historical commits for a YouTube devlog video. The manifest defines ~20 clips across 4 "acts" of development.

---

## 2. Cross-Platform Feasibility

### OBS availability

| Platform | OBS Studio | WebSocket built-in? | Notes |
|----------|-----------|---------------------|-------|
| **Linux** | ✅ | ✅ (28+) | Native package or Flatpak. PipeWire for Wayland, Xcomposite for X11. |
| **Windows** | ✅ | ✅ (28+) | DirectX capture. Most mature platform for OBS. |
| **macOS** | ✅ | ✅ (28+) | Screen Capture API. Deprecated old method, newer macOS (13+) uses different capture path. |

**Verdict**: OBS itself runs on all three desktop platforms. WebSocket support is standard.

### Screen capture APIs — The real challenge

The screen capture implementation is **deeply platform-specific**. OBS exposes different source types per platform:

| Capture Method | Linux | Windows | macOS |
|---------------|-------|---------|-------|
| PipeWire (Wayland) | ✅ | ❌ | ❌ |
| Xcomposite (X11) | ✅ | ❌ | ❌ |
| DirectX / BitBlt / WGC | ❌ | ✅ | ❌ |
| macOS Screen Capture | ❌ | ❌ | ✅ |
| Display Capture (AVFoundation) | ❌ | ❌ | ✅ |

Our current code (`obs_controller.py`) specifically handles PipeWire screen capture with RestoreToken persistence. This is Linux/Wayland-only.

**To be cross-platform, we'd need**:
- Windows: Create a "Window Capture (BitBlt)" or "Game Capture" source (different input kind string, different settings)
- macOS: Create a "Display Capture" or "Window Capture" source (different API entirely)
- Linux/X11: Create an "Xcomposite" source (different input kind)

Each of these has **different settings, different persistence mechanisms, and different behaviors** (e.g., Windows window capture can auto-match by executable name, macOS requires permission dialogs, Wayland needs xdg-desktop-portal).

### Python dependency

`obsws-python` is a **pure Python** library (only depends on `websocket-client` and `tomli`). It works on any platform with Python 3.9+. No native compilation needed.

**Issue**: Requiring a Python runtime + pip dependencies is a heavy ask for a Godot addon. End-users would need to:
1. Install Python 3.9+
2. `pip install obsws-python`
3. Have OBS Studio installed with WebSocket enabled

### Cross-platform summary

| Concern | Linux | Windows | macOS |
|---------|-------|---------|-------|
| OBS install | Easy (package mgr) | Easy (installer) | Easy (DMG) |
| WebSocket | Built-in 28+ | Built-in 28+ | Built-in 28+ |
| Python | Usually pre-installed | Manual install | Usually pre-installed |
| Screen capture | PipeWire / Xcomposite | DirectX / BitBlt / WGC | macOS Screen Capture |
| Window matching | Limited on Wayland | Full (exe name, title) | Partial |
| **Verdict** | ✅ Good (what we have) | ⚠️ Needs different impl | ⚠️ Needs different impl |

**Bottom line**: A public addon would need **platform-conditional capture source creation**. The OBS control part (start/stop recording, scene management) is already cross-platform via WebSocket. The capture source setup must be platform-aware.

---

## 3. Existing Tooling Landscape

### Directly relevant projects

#### 1. obs-websocket-gd (you-win) — ⭐ 116 stars
- **Repo**: https://github.com/you-win/obs-websocket-gd
- **Language**: Pure GDScript
- **What it does**: Implements the OBS WebSocket v5 protocol natively in GDScript. Lets you control OBS (start/stop recording, switch scenes, etc.) from within a Godot game or app.
- **Godot version**: 4.0.x
- **License**: Apache-2.0
- **Notes**: This is the closest existing project. It's a runtime-level addon (add to a scene, control OBS from game code). Not an editor plugin. Has a `send_command()` API and signal-based event handling. Would be an excellent foundation for an editor plugin.

#### 2. Godot OBS Recorder (Ryash)
- **Asset Library**: Godot 3.5, Tools category
- **Repo**: https://github.com/ryash072007/Godot-OBS-Recorder
- **What it does**: Editor plugin that lets you connect to OBS from the Godot editor. Has UI for connection settings, password, etc.
- **Godot version**: 3.5 only (archived/unmaintained)
- **License**: MIT
- **Notes**: This is the closest to what we're considering — an editor plugin. But it's Godot 3-only and appears unmaintained. Could be a reference for UX patterns.

#### 3. godot-recorder / GodotRecorder (henriquelalves)
- **Repo**: https://github.com/henriquelalves/GodotRecorder
- **What it does**: Records frames internally within Godot using `RenderingServer` to capture viewport to image sequences. No OBS involvement.
- **Notes**: Completely different approach — captures frames from within the engine. Avoids OBS dependency entirely but has its own overhead. Good for comparison.

#### 4. pylibobs (jonata)
- **Repo**: https://github.com/jonata/pylibobs
- **What it does**: Python bindings for libobs — lets you run OBS headlessly from Python. No GUI needed.
- **License**: GPL-2.0+
- **Notes**: Interesting alternative approach: embed OBS directly in Python without needing the OBS application running. But GPL license is restrictive, and it's beta-quality.

### Related community discussion

- A Reddit post (r/godot) by the author of `obs-websocket-gd` shows interest in both runtime and editor use: "Useful for recording short demos, tutorial snippets, or for streaming!"
- Godot Forum user reports issues with embedded game windows and OBS capture (Godot 4.4+).
- General consensus in Godot community: OBS is the recommended external tool for recording; Godot's built-in Movie Maker mode is for pre-recorded/trailer captures.

### Gap analysis

| Need | Existing solutions | Gap |
|------|------------------|-----|
| OBS control from GDScript | ✅ `obs-websocket-gd` | Works at runtime, not editor |
| Editor plugin for recording | ⚠️ Godot OBS Recorder (3.5 only) | No maintained Godot 4 solution |
| Automated multi-commit capture | ✅ Our scripts/ | Linux-only, Python-dependent |
| One-click "record my game" from editor | ❌ Nothing maintained | Clear gap |
| Cross-platform screen capture setup | ❌ Not in any existing tool | Hard problem |

**Verdict**: There's no maintained, cross-platform, Godot 4 editor addon for OBS recording. This is a genuine gap.

---

## 4. Editor Integration Approaches

We identified three architectural approaches, in order of increasing Godot-native-ness:

### Approach A: Existing Python pipeline + EditorPlugin wrapper

Keep the Python scripts as-is. Create a thin EditorPlugin that calls them via `OS.execute()`.

```
Godot EditorPlugin
  ├── UI button: "Record Current Scene"
  └── OS.execute("python3", ["scripts/capture_obs.py", current_scene_path, ...])
```

**Pros**:
- Minimal new code — reuses existing, tested pipeline
- All the OBS logic (source creation, recording, file management) stays in Python
- Editor plugin is just a launcher

**Cons**:
- Requires Python + obsws-python on user's machine (heavy dependency)
- Linux-only (current Python code)
- Error handling is indirect (parse stdout from subprocess)
- Feels bolted-on, not native

### Approach B: Pure GDScript addon (using obs-websocket-gd)

Port the OBS control logic into GDScript using `obs-websocket-gd` as the WebSocket layer. The addon handles everything natively.

```
addons/godot-obs-recorder/
├── plugin.cfg
├── plugin.gd              # EditorPlugin entry point
├── obs_controller.gd      # OBS WebSocket wrapper in GDScript
├── obs_recorder_dock.gd   # UI dock panel
├── obs_recorder_dock.tscn # Dock scene
└── platform/
    ├── capture_linux.gd   # PipeWire capture source setup
    ├── capture_windows.gd # DirectX/Windows capture setup
    └── capture_macos.gd   # macOS capture setup
```

**Pros**:
- Zero external dependencies — pure Godot
- Works on all platforms (with per-platform capture source logic)
- Full integration: dock panel, signals, EditorInterface integration
- Can be published on Asset Library
- Users just download, enable, and configure

**Cons**:
- Must reimplement OBS WebSocket auth/handshake (or vendor `obs-websocket-gd`)
- Must implement capture source creation for each platform
- GDScript WebSocket performance is fine for control messages (not video)
- Larger surface area to maintain

### Approach C: Hybrid — GDScript plugin + bundled Python helper

The Godot plugin provides the UI and triggers, but ships a small bundled Python script (or optional) for the heavy lifting.

```
addons/godot-obs-recorder/
├── plugin.gd              # EditorPlugin — settings, UI, triggers
├── obs_dock.gd            # Dock panel UI
├── scripts/               # Bundled Python scripts (optional runtime)
│   ├── capture_obs.py     # (same as current, but configurable)
│   └── obs_controller.py
├── platform/
│   └── ...                # Per-platform capture helpers
└── README.md
```

**Pros**:
- Best of both worlds
- Python path is optional power-user feature

**Cons**:
- Still has Python dependency for the automated path
- Two code paths to maintain

### Recommendation

**Approach B (Pure GDScript)** is the right target for a public addon. It's the lowest friction for users — install, enable, configure, record. No external dependencies.

**Approach A** is fine for our internal use (what we have now).

We could **start with A** for our immediate needs and **evolve toward B** for release.

---

## 5. Architecture Brainstorming

### Feature wishlist

- **One-click recording** from the Godot editor toolbar
- **Auto-detect OBS** — check if OBS is running, optionally start it
- **Scene-aware** — record the current scene or a specific test scene
- **Output management** — choose output directory, naming conventions
- **Encoding presets** — resolution, FPS, codec (proxy vs. final)
- **Timeline markers** — save bookmarks during recording for later editing
- **Batch recording** — record multiple scenes/tests sequentially (similar to our capture_all_showcase.sh but from within Godot)

### Proposed addon structure

```
addons/godot-obs-recorder/
├── plugin.cfg                          # Addon metadata
├── plugin.gd                           # EditorPlugin: init, dock, toolbar
├── icons/
│   └── icon_recorder.svg               # Addon icon
├── src/
│   ├── obs_websocket_client.gd         # OBS WebSocket communication
│   ├── obs_recorder_dock.gd            # Main dock panel logic
│   ├── obs_recorder_dock.tscn          # Dock panel scene
│   ├── obs_settings.gd                 # Settings resource
│   ├── obs_settings.tres               # Default settings
│   ├── platform/
│   │   ├── capture_linux_pipewire.gd   # PipeWire source creation
│   │   ├── capture_windows_directx.gd  # Windows DirectX/BitBlt
│   │   └── capture_macos_screen.gd     # macOS Screen Capture
│   └── util/
│       ├── godot_process_helpers.gd    # Launch/manage Godot instances
│       └── file_naming.gd              # Output path helpers
├── scripts/                            # Optional CLI helpers
│   ├── capture_scene.py
│   └── requirements.txt
└── README.md
```

### Key design decisions to explore

#### 1. Where does recording happen?

- **Option A: Record the running editor's game view**
  - Capture the "Game" tab / embedded game window
  - Issues: embedded game window is tricky for OBS capture (known Godot 4.4+ issue)
  - Pro: no separate process, captures exactly what you see

- **Option B: Launch a separate instance and capture that**
  - What our current pipeline does: `godot --scene <path> --max-fps <n>`
  - Pro: clean capture, no editor UI in the recording
  - Con: need to wait for launch, manage process lifecycle

- **Option C: Record the running game from an already-running instance**
  - If the game is already running, just capture its window
  - Pro: captures real gameplay, not a fresh launch
  - Con: window must be identifiable to OBS

#### 2. How to handle the screen capture source?

This is the hardest cross-platform challenge. Each platform needs different OBS source creation:

**Linux (PipeWire/Wayland)**:
- Source kind: `pipewire-screen-capture-source`
- Settings: `{ "RestoreToken": "<uuid>" }`
- Challenge: token must be obtained via xdg-desktop-portal interaction
- Our current approach: hardcoded fallback token, persist to file

**Linux (X11/Xcomposite)**:
- Source kind: `xcomposite_screen` / `monitor_capture`
- Settings: varies by monitor selection
- Challenge: window selection by title/exe for automatic capture

**Windows**:
- Source kind: `window_capture` (BitBlt) or `game_capture`
- Settings: `{ "window": "<hwnd or title match>", "capture_method": "bitblt" }`
- Pro: can match by executable name — "godot" / "godot_*" patterns work well
- Challenge: OBS runs as admin? Godot window visibility?

**macOS**:
- Source kind: `display_capture` or `window_capture` (macOS Screen Capture)
- Settings: permission prompt required on macOS 10.15+ (Screen Recording permission)
- Challenge: permission model is restrictive, need user to grant access

#### 3. Configuration UI

A simple dock panel with:

```
┌─────────────────────────────┐
│  OBS Recorder               │
│                             │
│  Status: ● Connected (v31)  │  ← connection status indicator
│                             │
│  ┌─────────────────────┐    │
│  │ OBS Host:  localhost │    │
│  │ OBS Port:  4455      │    │
│  │ Password:  ●●●●●●●●  │    │
│  └─────────────────────┘    │
│                             │
│  [Test Connection]          │
│                             │
│  ──── Recording ────       │
│  Scene:  [MyScene ▼]       │  ← OBS scene selector
│  Duration: [30] seconds     │
│  FPS cap:  [60 ▼]          │
│  Output:  [Browse...]       │
│                             │
│  [● Start Recording]        │  ← contextual: Start/Stop/Cancel
│                             │
│  ──── Batch ────            │
│  [Config...] [Run Batch]    │  ← manifest-based batch recording
│                             │
│  Recent captures:           │
│  ├ 2026-07-28_campfire.mp4  │
│  └ 2026-07-28_stress.mp4   │
└─────────────────────────────┘
```

#### 4. OBS discovery and lifecycle

How does the addon find OBS?

- **Auto-discovery**: Try default WebSocket port (4455) on localhost. If it responds, OBS is running.
- **Launch on demand**: Optionally launch OBS from known install paths (platform-specific).
  - Linux: `obs` (from PATH), `/usr/bin/obs`, Flatpak
  - Windows: `C:\Program Files\obs-studio\bin\64bit\obs64.exe`
  - macOS: `/Applications/OBS.app/Contents/MacOS/OBS`
- **Manual config**: User provides host:port:password.

---

## 6. Open Questions & Next Steps

### Questions to resolve

1. **License compatibility**: `obs-websocket-gd` is Apache-2.0. If we vendor/distribute it, compatible with MIT/GPL? Our project is Apache-2.0 too, so fine.

2. **OBS WebSocket protocol complexity**: The WebSocket auth handshake (identify with HMAC-SHA256) is non-trivial in GDScript. Does `obs-websocket-gd` handle this correctly for v5? (Yes, it targets obs-websocket 5.x.)

3. **Wayland token capture UX**: On Wayland, the first capture setup requires an interactive xdg-desktop-portal dialog (user selects monitor). How do we handle this from within Godot? Portal doesn't work headless. Could:
   - Prompt user to set up the capture source manually once in OBS
   - Then the addon's token persistence remembers it
   - This is what our current code does (token persistence)

4. **Godot 4 built-in Movie Maker**: Godot has a "Movie Maker" mode that writes video directly. Why not use it instead?
   - Movie Maker uses the rendering pipeline directly, which **bypasses the game's real-time loop** — it renders each frame as fast as possible, not at real-time speed
   - For footage that needs real-time game feel (not "render for trailer"), OBS capture is better
   - Movie Maker can bottleneck on the CPU planning thread just like the native monitor
   - See https://docs.godotengine.org/en/stable/tutorials/animation/creating_movies.html

5. **Editor plugin vs. runtime addon**: Our use case is primarily **editor-based** (recording footage during development). But the addon could also work at runtime (e.g., player recording/streaming from within the game). Should we target both?

### Proposal for next iteration

1. **Short-term** (our internal use):
   - Keep the Python pipeline as-is for batch capture workflows
   - Add a simple EditorPlugin that wraps `capture_obs.py` with a one-click button
   - Make it Linux-only, explicitly documented

2. **Medium-term** (possible public addon):
   - Port the OBS WebSocket communication to GDScript (based on `obs-websocket-gd`)
   - Implement platform-specific capture source creation
   - Build the dock panel UI
   - Release as `godot-obs-recorder` on GitHub + Asset Library

---

*This document was generated as part of an exploration session. See `notes/obs-recorder-addon/` for all related artifacts.*
