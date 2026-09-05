# Logging Slim-Down Proposal

**Date**: 2026-09-03 **Context**: Verbosity audit of Rust planner + GDScript autoload logging **Status (2026-09-04)**: Implemented. Baseline 35 s campfire_2d headless at default Debug: 59,176 lines (find_candidates 3,446; Processing callback 1,512; process_simulation 1,594; NOT resuming 15 post-time-slice). After (default Info): 131 lines — 19× `submit_plan` / `Plan complete` / `RESULT` triples only, zero debug spam, zero debugger errors. Debug opt-in (15 s): 316 lines, tree dumps gone (recording still on, fetched via `get_debug_tree()`). Per-iteration detail removed, not demoted — see "Implementation" below.

______________________________________________________________________

## Problem

~222k log lines / ~23 MB per 60 s at default `log_level=3` (Debug). Noisiest sites dominate output and drown out plan summaries.

______________________________________________________________________

## Keep at Info

- `scheduler.rs` `submit_plan` summary (agent, goal, action count).
- `Plan complete` one-liner on success/failure.
- `debug_tree.rs:404`-style one-liner: `RESULT / Branches / Time / goal`.
- Errors and warnings only (failed callbacks, invalid plans).

______________________________________________________________________

## Demote, Gate, or Throttle

- `expander.rs:496/498` `find_candidates ready` (~15k hits) -> `trace!` or counter.
- `scheduler.rs:170` `Processing callback` handshake -> `trace!`.
- `scheduler.rs:206` `NOT resuming` (938 hits/60 s) -> throttle to one line per plan tick.
- `Pending / Resuming` transitions -> `trace!` or debug-gated flag.
- `engine.rs` `process_simulation` payload dumps -> `trace!`.
- Full `PLANNER SEARCH TREE` dump with `Try` lines (440 `Add Fuel` hits) -> remove from hot path; keep behind `get_debug_tree()` only.

______________________________________________________________________

## Default Level and Teardown Guard

- Change `plugin.cfg:13` default `log_level` 3 (Debug) -> 2 (Info). Risk: users relying on Debug output lose detail; mitigate with documented opt-in flag.
- Guard `gdpai_autoload.gd:16` `EngineDebugger.send_message` with `EngineDebugger.is_active()` to fix headless `ERROR`. Godot exit teardown noise is unrelated; leave out of scope.

______________________________________________________________________

## File:Line Checklist

- `addons/GdPlanningAI/plugin.cfg:13` default level 3 -> 2.
- `addons/GdPlanningAI/rust/src/scheduler.rs:170` callback -> trace.
- `addons/GdPlanningAI/rust/src/scheduler.rs:206` NOT resuming -> throttle.
- `addons/GdPlanningAI/rust/src/scheduler.rs` submit_plan vs Plan complete -> keep Info.
- `addons/GdPlanningAI/rust/src/expander.rs:496/498` find_candidates -> trace.
- `addons/GdPlanningAI/rust/src/engine.rs` process_simulation + tree.format -> gate.
- `addons/GdPlanningAI/rust/src/debug_tree.rs:404` keep one-line summary.
- `addons/GdPlanningAI/gdpai_autoload.gd:16` guard send_message.

______________________________________________________________________

## Implementation (2026-09-04)

- `find_candidates` detail (expander), `process_simulation` payloads (engine), and the `Processing callback` handshake (scheduler) were first demoted to a new `LogLevel::Trace`, then removed entirely on review: the tree already records every candidate with node context, dead ends, and open/satisfied needs, so the flat per-iteration stream answered no question the tree couldn't. No `log_trace!` / `Trace` remains.
- The one non-redundant trace signal — which requirement failed verification — is preserved as an `FWD [FAIL]` step on the failing tree node (`simulate_and_advance`, now `&mut self`), visible via `get_debug_tree()` at zero log volume.
- `Pending` / `Resuming` stay at Debug (low volume, stall-relevant). `NOT resuming` stays at Debug but throttled to one line per stall episode via `StallNoticeThrottle` (deliberately not silenced: each genuine stall still logs once).
- `submit_plan` promoted to Info with goal names; new Info one-liner `RESULT goal='…' success=… Branches: … | Time: …` at both completion sites (engine), backed by an always-on branch counter (`TreeDump::summary`). Full tree never auto-prints; recorded at Debug and fetched via `get_debug_tree()`.
- Default `plugin.cfg` 3 -> 2; `EngineDebugger.send_message` guarded by `is_active()`.
- Tests: `tests/logging_levels.rs` (Info completes with summary but no tree; Debug re-enables dump; FWD FAIL attribution; throttle; disabled-tree counting), throttle unit tests in `scheduler.rs`, logger mapping tests, GUT `test_log_level_config.gd` pins the default.
- Docs: `ALGORITHM.md` §11, `PLANNER_PSEUDOCODE.md` §11, `AUTHORING_GUIDE.md` debugging paragraph, `CODEBASE_OVERVIEW.md` logger row.
