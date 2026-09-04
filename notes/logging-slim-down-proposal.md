# Logging Slim-Down Proposal

**Date**: 2026-09-03 **Context**: Verbosity audit of Rust planner + GDScript autoload logging

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
