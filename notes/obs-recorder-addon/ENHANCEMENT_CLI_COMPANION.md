# GdTimeMachine Historical Capture (CLI Companion)

Status: Core differentiator (design from v0.1, implement after core addon stabilizes)

---

## Why this matters

Most Godot recording tools can only capture the current editor state. GdTimeMachine's
standout feature is being able to rewind your project to any commit and record
it — automatically resolving the correct Godot version, rebuilding native
extensions, and regenerating resource caches.

This is a **time machine for your project's visual history**. It's what makes
GdTimeMachine more than a record button: it's a tool for telling the story of your
game's development.

## The problem

GdTimeMachine's editor dock can record the current scene, but it cannot:
- Check out historical git commits
- Resolve + install the correct Godot version for an old commit (via godotenv)
- Rebuild Rust GDExtensions at old commits
- Regenerate `.godot/` resource caches
- Run batch captures across multiple commits

These operations require the Godot editor to **not** be running in the project
directory, making them impossible to execute from inside an editor plugin.

## The solution

A standalone CLI tool (bundled with GdTimeMachine but executed outside the editor) that:

1. Reads a **batch manifest** created by GdTimeMachine's dock UI
2. For each entry: git worktree → resolve godotenv → rebuild Rust → regenerate .godot/ → record
3. Reports results back to GdTimeMachine (via exit code + log file)

## Manifest format

GdTimeMachine's dock UI writes this JSON. The CLI reads it.

```json
// gdclip_batch_20260729.json
{
  "project_root": "/home/user/projects/my-game",
  "godot_path": "godot",
  "obs": {
    "host": "localhost",
    "port": 4455,
    "password_env_var": "OBS_PASSWORD",
    "scene_name": "Scene"
  },
  "captures": [
    {
      "commit": "dc48fe5",
      "scene": "examples/hunger_basic_2d.tscn",
      "label": "01_before_pure_gdscript",
      "act": "act1_foundation",
      "duration": 30,
      "fps": 60,
      "fullscreen": true,
      "godot_version_hint": "4.3"
    },
    {
      "commit": "a9bda00",
      "scene": "examples/campfire_2d.tscn",
      "label": "09_final_campfire",
      "act": "act4_polish",
      "duration": 30,
      "fps": 60,
      "fullscreen": true
    }
  ]
}
```

## CLI usage

```bash
# Basic usage — reads manifest, runs all captures
gdclip-cli run gdclip_batch_20260729.json

# Dry run — show what would be captured without recording
gdclip-cli run --dry-run manifest.json

# Resume from a specific label (after a failure)
gdclip-cli run --resume 05_rust_suite_passing manifest.json

# List available commits (given a manifest template)
gdclip-cli list-commits manifest.json
```

## What the CLI does per entry

```
For each entry in manifest.captures:
  1. git worktree add .worktrees/<label> <commit>
  2. Resolve Godot binary (via godotenv or PATH)
  3. If Rust present: cargo build (debug) → copy .so
  4. Remove .godot/ → godot --editor --quit (regenerate cache)
  5. Symlink GdTimeMachine's Python scripts into the worktree
  6. Launch: capture_obs.py --scene <scene> -d <duration> -o <output> -f --start-obs
  7. Wait for completion
  8. Clean up worktree
```

## Integration with GdTimeMachine

GdTimeMachine's dock UI provides:

```
┌──────────────────────────────────┐
│  ● Batch Recording               │
├──────────────────────────────────┤
│  ┌────────────────────────────┐  │
│  │ Manifest entries:          │  │
│  │ ├ [main] campfire_2d.tscn  │  │
│  │ ├ [1a2b3c] hunger_basic    │  │
│  │ └ [def456] stress_test     │  │
│  │                            │  │
│  │ [+ Add Entry] [Remove]     │  │
│  └────────────────────────────┘  │
│                                   │
│  [Export Manifest...]             │  → writes JSON file
│  [Run in Terminal (copy cmd)]     │  → copies CLI command to clipboard
│                                   │
│  Previous runs:                   │
│  ├ batch_20260728_1430 ✅ all 3/3│
│  └ batch_20260727_0900 ⚠ 2/3    │
└──────────────────────────────────┘
```

GdTimeMachine never executes the batch — it configures, exports, and tracks results.
The CLI owns execution.

## Language choice

Two options for the CLI tool:

| Option | Pros | Cons |
|--------|------|------|
| **Python** (bundled venv) | Reuses our existing `capture_obs.py` and `obs_controller.py` directly. Minimal new code. | Requires Python 3.9+ on user's machine. |
| **Rust** (single binary) | Zero dependencies. Cross-platform binary. Matches the project's Rust tooling. | Rewrite the OBS WebSocket logic from Python. More effort for first version. |

**Recommendation**: Python for v1 (our scripts already work), Rust if there's demand for a self-contained binary later.

## Relationship to GdPlanningAI

The existing `capture_all_showcase.sh` is purpose-built for **GdPlanningAI's devlog** with a hardcoded manifest of commits. The CLI companion generalizes this into a reusable tool that works for any Godot project, with the manifest created by GdTimeMachine's UI.

Long-term, `capture_all_showcase.sh` either gets replaced by `gdclip-cli run` or lives on as a thin wrapper with the GdPlanningAI-specific manifest.

---

## Effort estimate

| Part | Time | Notes |
|------|------|-------|
| JSON manifest schema + validation | 1–2 hrs | Shared with GdTimeMachine dock |
| `gdclip-cli run` (core loop) | 3–4 hrs | Port of `capture_all_showcase.sh` logic |
| Worktree management | 1 hr | Already solved in the shell script |
| Godot binary resolution | 1 hr | godotenv integration |
| Rust build integration | 1 hr | Same as current script |
| Dry-run, resume, error handling | 2 hrs | UX polish |
| **Total** | **9–11 hrs** | |

## Dependencies

- GdTimeMachine v0.1+ (defines manifest format)
- Python 3.9+ (for v1)
- `obsws-python` (same as GdTimeMachine's OBS backend)
- Git (for worktree operations)
- Cargo (for Rust rebuilds — optional per project)
