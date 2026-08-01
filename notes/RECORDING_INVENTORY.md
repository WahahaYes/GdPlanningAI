# Recording Infrastructure Inventory (pre-GdTimeMachine)

Date: 2026-08-01 Status: Reference for cleanup

## Purpose

GdPlanningAI contains an OBS-based showcase recording setup that predates the split-off of GdTimeMachine (a sibling repo, next door at `../GdTimeMachine`). This note inventories every recording-related file so they can be referenced, then cleared out once GdTimeMachine's addon reaches parity and replaces this functionality.

Nothing here is deleted yet — this is the tracking document.

______________________________________________________________________

## Recording scripts (`scripts/`)

| Path | Tracked | Purpose | |------|---------|---------| | `scripts/capture_obs.py` | Yes | Main OBS recording script; launches a Godot scene, controls OBS via WebSocket, captures video to `media/captures/` | | `scripts/obs_controller.py` | Yes | OBS WebSocket controller; manages screen-capture sources, RestoreToken persistence, recording start/stop | | `scripts/capture_all_showcase.sh` | Yes | Batch recording script; uses git worktrees to check out historical commits and record showcase footage from each | | `scripts/.obs_capture_token.json` | No (ignored) | Persisted OBS PipeWire RestoreToken for Wayland screen-capture sessions |

## Configuration

| Path | Tracked | Purpose | |------|---------|---------| | `.env` | No (ignored) | OBS WebSocket credentials (`OBS_HOST`, `OBS_PORT`, `OBS_PASSWORD`); sourced by Makefile record targets. Contains secrets — never commit | | `.env.example` | Yes | Template for `.env` with setup instructions |

## Git worktrees (`.worktrees/`)

| Path | Tracked | Purpose | |------|---------|---------| | `.worktrees/` | No (ignored) | Worktrees for each showcase commit; lets the capture script record historical scenes without touching the main working tree | | `.worktrees/01_before_pure_gdscript/` | No | Worktree at commit `dc48fe5` (pre-Rust state) | | `.worktrees/01b_multi_agent/` | No | Multi-agent variant of `dc48fe5` | | `.worktrees/01c_stress_test/` | No | Stress-test variant of `dc48fe5` | | `.worktrees/02_first_rust_demos_working/` | No | Worktree at commit `3429296` | | `.worktrees/02b_multi_agent/` | No | Multi-agent variant of `3429296` | | `.worktrees/03_rust_tests_exist/` | No | Worktree at commit `7b7b968` | | `.worktrees/03b_multi_agent/` | No | Multi-agent variant of `7b7b968` | | `.worktrees/04_reimplementation_status/` | No | Worktree at commit `6947d24` | | `.worktrees/cargo-target/` | No | Shared Rust build target directory for worktree builds | | `.worktrees/test_dc48fe5/` | No | One-off test worktree |

## Captured media (`media/captures/`)

| Path | Tracked | Purpose | |------|---------|---------| | `media/captures/` | No (ignored) | All recorded video output | | `media/captures/act1_foundation/` | No | Act 1 showcase clips (pre-Rust era) | | `media/captures/act2_rust_leap/` | No | Act 2 showcase clips | | `media/captures/act3_rewrites/` | No | Act 3 showcase clips | | `media/captures/act4_polish/` | No | Act 4 showcase clips | | `media/captures/*.mp4` | No | Standalone test recordings (incl. a 122 MB stray capture) |

Note: `media/*.png|*.gif` (banner, screenshots, demo GIFs) are README assets, **not** recording output — keep those.

## Makefile targets

| Target | Purpose | |--------|---------| | `record-obs` | Records a scene via OBS; auto-loads `.env`, invokes `scripts/capture_obs.py` | | `record-obs-fullscreen` | Same, with `-f` fullscreen flag |

Defaults: `SCENE=examples/hunger_basic_2d.tscn`, `DURATION=10`, `FPS=60`, `OUTPUT=media/captures`.

## Design docs for the replacement (`notes/obs-recorder-addon/`)

These are the design birthplace of GdTimeMachine — **preserve, do not delete**.

| Path | Tracked | Purpose | |------|---------|---------| | `notes/obs-recorder-addon/ARCHITECTURE.md` | Yes | GdTimeMachine addon architecture sketch | | `notes/obs-recorder-addon/IMPLEMENTATION_PLAN.md` | Yes | Phased implementation plan for the addon | | `notes/obs-recorder-addon/BRAINSTORM.md` | Yes | Feature brainstorming scratchpad | | `notes/obs-recorder-addon/ENHANCEMENT_CLI_COMPANION.md` | Yes | Design for the CLI companion (historical capture) | | `notes/obs-recorder-addon/RESEARCH.md` | Yes | OBS WebSocket / capture-method research |

## Related

- `notes/YOUTUBE_SHOWCASE_COMMITS.md` — commit timeline used for the showcase recording plan (references the worktrees/acts above)
- `examples/` scenes (`hunger_basic_2d.tscn`, `hunger_multi_agent_2d.tscn`, `hunger_stress_test_2d.tscn`, `campfire_2d.tscn`, `campfire_3d.tscn`) — the scenes used as recording subjects; keep, they are the project's demos
- Git history: `925c360` "recording via obs", `3144bc2` "setting up commit record", `bcfac9d` "clean up recording scripts"

## Gitignore rules covering the untracked items

- `.env` — `.gitignore` line 34
- `scripts/.obs_capture_token.json` — `.gitignore` line 38
- `media/captures/` — `.gitignore` line 31
- `.worktrees/` — `.gitignore` line 44

## GdTimeMachine parity check (as of 2026-08-01)

What GdTimeMachine already has: `backend/recorder_backend.gd` (abstract base), `backend/backend_movie_maker.gd` (Movie Maker backend), dock UI, plugin.

What GdTimeMachine **lacks** vs this setup:

| Capability | Here | GdTimeMachine | |-----------|------|---------------| | OBS WebSocket capture | `scripts/capture_obs.py` + `obs_controller.py` | Not yet | | Historical commit recording via worktrees | `capture_all_showcase.sh` + `.worktrees/` | CLI companion designed, not implemented | | Scene-based recording from Makefile | `record-obs` targets | Not yet | | Credential/config handling | `.env` + `.env.example` | Not yet |

## Cleanup checklist (do this when GdTimeMachine reaches parity)

1. Confirm GdTimeMachine addon can record via OBS and Movie Maker from within the editor, and its CLI companion can do worktree-based historical capture.
1. Delete scripts: `scripts/capture_obs.py`, `scripts/obs_controller.py`, `scripts/capture_all_showcase.sh` (+ untracked `scripts/.obs_capture_token.json`).
1. Delete `.env`/`.env.example` and the `record-obs`/`record-obs-fullscreen` Makefile targets (+ their `SCENE`/`DURATION`/`FPS`/`OUTPUT` variables).
1. Delete `.worktrees/` (prune worktrees first via `git worktree prune`).
1. Delete `media/captures/` after archiving any footage worth keeping.
1. Retire `.gitignore` entries only if nothing else needs them (`.env`, `.obs_capture_token.json`, `media/captures/`, `.worktrees/`).
1. Keep `notes/obs-recorder-addon/` and `notes/YOUTUBE_SHOWCASE_COMMITS.md` unless folded into the GdTimeMachine repo.
