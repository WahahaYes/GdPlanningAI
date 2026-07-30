# OBS Recorder Addon — Brainstorming Scratchpad

Quick notes, ideas, and sketches. Not polished.

---

## Name Ideas

- Godot OBS Recorder (boring but descriptive)
- Godot Viewport Capture
- OBSync (OBS + sync)
- Godot Broadcaster
- Obsidian Recorder
- SceneCap

## Wilder Ideas

### "Record That" button
What if the addon added a small button to the **debugger toolbar** or **scene tree dock** that does "record last N seconds"? Like NVIDIA Shadowplay but for Godot dev. Uses OBS replay buffer.

```
OBS replay buffer:
  obs.start_replay_buffer()     # keeps last 30s in memory
  obs.save_replay_buffer()       # triggered by button → saves last 30s to disk
```

Zero overhead until you hit save. Perfect for capturing unexpected emergent AI behavior without planning ahead.

### CI integration
The batch recording workflow (`capture_all_showcase.sh`) could be adapted to run in CI:
- Spin up Godot in headless-ish mode
- Use OBS virtual camera or direct encoder output
- Capture footage as CI artifacts for PR review
- "Visual diff" — compare screenshots frame-by-frame?

Probably overkill but interesting.

### Python-free fallback
For users who can't/won't install Python: fall back to `obs-websocket-js` via Node.js, or even a simple shell script that calls `curl` against the OBS WebSocket REST-like endpoints.

Actually, OBS WebSocket is a real WebSocket (not REST). Can't use curl. So you'd need Node or Python or GDScript.

### OBS installation detection

On Linux you might find OBS at:
```
/usr/bin/obs
/usr/bin/obs-studio
/var/lib/flatpak/app/com.obsproject.Studio/.../export/bin/com.obsproject.Studio
~/.local/bin/obs
```

Could use `OS.execute("which", ["obs"])` → check return code. Store in settings with a "Locate OBS..." file picker fallback.

### Potential pitfalls

1. **OBS WebSocket password** — How does the user configure it?
   - Hardcoded in addon settings (stored in project.godot — bad for sharing)
   - Environment variable (like our current `OBS_PASSWORD`)
   - System keychain via custom tool
   - Simplest: text field in addon settings with a warning tooltip

2. **OBS version detection** — OBS 28+ has WebSocket built-in. Older versions need plugin.
   - On connect, check `GetVersion` response
   - Warn if WebSocket version is < 5.0.0

3. **Multiple monitors** — Which monitor gets captured?
   - Current: full-screen Godot window → PipeWire capture captures the monitor Godot is on
   - Could let user pick monitor in OBS scene config
   - Or always capture the "Godot" window specifically (window capture, not screen capture)

4. **Godot embedded game window** — The new Godot 4 "embedded game" feature puts the game inside the editor as a sub-window. OBS can't easily capture just that sub-window. Known issue (godot#103154). Workaround: launch a separate instance.

5. **Audio** — Our current pipeline doesn't capture game audio. OBS can capture desktop audio, but we'd want:
   - Game audio separate from mic/desktop
   - Configurable audio sources
   - This is more of an OBS scene configuration concern than an addon concern

## OBS WebSocket GDScript notes

Key things we'd need to implement in GDScript (if not using obs-websocket-gd):

```gdscript
# OBS WebSocket v5 auth handshake:
# 1. Server sends "Hello" with authentication.challenge (base64) + authentication.salt (base64)
# 2. Client computes:
#    secret = base64_encode(sha256(password + salt))
#    auth_response = base64_encode(sha256(secret + challenge))
# 3. Client sends "Identify" with authentication_response field
# 4. Server responds with "Identified"

# Then for requests:
# Send: { "op": 6, "d": { "requestType": "StartRecord", "requestId": "uuid" } }
# Receive: { "op": 7, "d": { "requestType": "StartRecord", "requestId": "uuid", "requestStatus": {...} } }
```

OBS uses opcodes:
- 0: Hello
- 1: Identify
- 2: Identified
- 3: Reidentify
- 5: Event
- 6: Request
- 7: RequestResponse
- 9: BatchRequest (10: BatchResponse)

The GDScript `WebSocketPeer` can handle this — Godot 4 has good WebSocket support.

## Reference: obs-websocket-gd API surface

From reading the repo docs, the existing addon provides:

```
signals:
  connection_established()
  connection_authenticated()
  connection_closed()
  data_received(update_data: ObsMessage)

methods:
  establish_connection(host, port, password) -> void
  send_command(command: String, data: Dictionary) -> void
```

It uses GDScript + Godot's built-in `WebSocketClient`. This means we can use GDScript for OBS WebSocket and entirely avoid the Python dependency.

If we vendor this (it's Apache-2.0, same as our project), we could build our editor plugin on top of it directly.

---

## Timeline sketch

**Phase 1** (days): Wrap existing Python scripts with EditorPlugin → usable internally
**Phase 2** (weeks): Port WebSocket to GDScript → drop Python dependency
**Phase 3** (months): Platform capture sources, dock UI, batch UI → public release

---

_Just ideas, nothing committed to yet._
