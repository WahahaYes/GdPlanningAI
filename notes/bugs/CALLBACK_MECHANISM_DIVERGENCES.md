# Callback Mechanism Divergences

## Date
2025-06-14

## Context
While investigating campfire test timeouts (see `CAMPFIRE_TIMEOUT_CALLBACK_INVESTIGATION.md`),
we identified that the root cause is not a single infinite loop in `step_search`, but rather
three structural divergences between the intended callback design and the current
implementation. These divergences cause an exponential explosion of callback round-trips
that stalls the planner.

## Intended Design

```
┌──────────────┐      CallbackRequest      ┌──────────────┐
│  Rust Engine │  ───────────────────────►  │   Scheduler  │
│  (Rayon)     │         (mpsc)             │  (Main Thread)│
│              │                          │               │
│  step_search │◄─── PlannerCallback ──── │  dispatch     │
│    loop      │      (mpsc)              │  (Godot       │
│              │                          │  Callable)    │
└──────────────┘                          └──────────────┘
```

The Rust engine runs on a Rayon background thread and yields to Godot for any
dynamic evaluation it cannot perform in Rust (custom preconditions, cost
calculation, effect simulation). The flow:

1. **Engine** calls `eval_precondition`/`simulate_action` for a custom callable.
2. It sends a `CallbackRequest` through `request_tx` and immediately returns
   `Pending(request_id)`.
3. **Scheduler** (`process_callbacks`, called once per frame on the main thread)
   drains the request channel, invokes the GDScript `Callable` via
   `dispatch_callback`, and sends the result back as a `PlannerCallback`.
4. The engine is **resumed** only when the scheduler sees responses have arrived.
   At the top of `step_search`, it drains the response channel, caches the
   results, unparks affected nodes, and continues the A* loop.

**Key invariant**: a single `find_candidates` call can fire *many* callbacks, but
a node is **parked on only one request_id**. On the next resume, **all**
responses are drained from the channel and cached, so every pending callback
from the previous batch is answered simultaneously.

## Current Divergences

### Divergence 1: `find_candidates` re-fires already-pending callbacks

When a node is resumed with **one** response, `find_candidates` is called again.
It iterates **all** candidate actions. For every action whose discovery is still
not cached, it calls `simulate_action` again. If the supplied `response` doesn't
match what that specific action needs, it fires a **brand new callback** — even if
that same callback was already fired in a previous resume and is just waiting in
the pending map.

This creates a cascading explosion: each resume for a node spawns new pending
callbacks for the *other* candidates, which then spawn more on the next resume.

**Location**: `addons/GdPlanningAI/rust/src/planner/expander.rs`
- `get_discovery_result` checks `discovery_pending` but `find_candidates` still
  calls `simulate_action` unconditionally inside the per-action loop.
- The `response` parameter is passed down but only matches the *current* action's
  expected callback id; all other actions ignore it and fire fresh requests.

### Divergence 2: Nodes with ready candidates are still parked

In `engine.rs`:

```rust
for cand in candidates_res.ready {
    // ... expand children ...
}
if let Some(id) = candidates_res.pending_id {
    node.callback_response = None;
    self.parked_nodes.entry(id).or_default().push(node);
}
```

If `find_candidates` returns **both** ready candidates and a pending id, the node
is expanded **and** parked. When it is later resumed, it runs `find_candidates`
**again** and re-expands the same children. The `visited` check is skipped because
`node.resumed == true`. This creates duplicate branches and wasted work.

**Location**: `addons/GdPlanningAI/rust/src/planner/engine.rs` lines 298–345

### Divergence 3: A resumed node parks itself on a different callback id every time

`find_candidates` computes `last_pending_id` by overwriting it in the loop over
candidate actions. The node parks on whichever candidate happened to be evaluated
last. On the next resume, that specific response is attached to the node, but
`find_candidates` ignores it for all the *other* candidates and fires new ones.

This means the engine can never make clean forward progress: the node effectively
shuffles between different pending callbacks without ever exhausting them.

**Location**: `addons/GdPlanningAI/rust/src/planner/expander.rs` lines 138–140

## Why Campfire Tests Time Out

`CookPotato` and `DigPotato` have **3–4 custom validity checks** each plus **cost +
effect** callbacks. The planner needs **~6 callback round-trips** just to discover
them as candidates. Because of Divergence 1, each round-trip for one candidate
spawns fresh callbacks for the others. Divergence 2 causes the node to re-expand
and park repeatedly. Divergence 3 ensures the node never stabilises on a single
pending set.

The search makes almost no structural progress per resume, so a 6-branch
exploration that should take ~6 frames instead takes thousands of frames and
times out.

## Intended Fixes

### Fix 1: Reuse pending discovery requests

Before firing a new callback in `find_candidates`, check whether there is already
a pending request for the exact same `(action_idx, check_spec, bindings)` or
`(action_idx, bindings)` discovery. If pending, reuse that id and do **not**
re-fire.

This breaks the exponential growth by ensuring each unique callback is fired
exactly once.

### Fix 2: Only park nodes that produced zero ready candidates

In `engine.rs`, if a Searching node produced **any** ready candidates, do **not**
park it. Only park nodes that produced **zero** ready candidates.

This prevents duplicate expansion and ensures the search advances on every
resume.

### Fix 3: Resume with a merged cache, not a single response

When a resumed node re-enters `find_candidates`, it should use a **merged** set
of cached responses. The current code passes `node.callback_response` (a single
`CallbackResponse`) into `find_candidates`, which only matches one action's
expected id. Instead, `find_candidates` should read all available responses from
`ctx.discovery_precond_results` and `ctx.discovery_results` directly — these
are already populated by the engine at the start of `step_search`.

The `response` parameter can be removed or restricted to simulation-time uses only.

## Files to Modify

- `addons/GdPlanningAI/rust/src/planner/expander.rs`
  - Check pending maps before firing new callbacks in validity checks.
  - Check pending maps before firing new callbacks in discovery simulation.
  - Remove or narrow the `response` parameter in `find_candidates`.
- `addons/GdPlanningAI/rust/src/planner/engine.rs`
  - Only park a node when `candidates_res.ready.is_empty()`.
  - Ensure all cached responses are drained before entering the main loop.

## Verification Plan

1. Build with `make build-release`.
2. Run isolated campfire test:
   ```bash
   godot --headless -s --path . addons/gut/gut_cmdln.gd \
     -gtest=res://test/integration/test_campfire_example_smoke.gd \
     -gdir=res://does_not_exist -ginclude_subdirs=false \
     -gexit > campfire_callback_fix.log 2>&1
   ```
3. Check that "Resuming search" count is bounded (should be ~6 for a single
   `test_full_cooking_chain`).
4. Check that total per-goal time is under 200ms (not 14 seconds).
5. Check that `Cook Potato` and `Dig Potato` appear in the debug tree.
